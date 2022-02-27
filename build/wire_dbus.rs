use crate::open;
use crate::tokens::{tokenize, Lined, Symbol, Token, TokenKind, TreeDelim};
use anyhow::{bail, Context, Result};
use bstr::{BStr, BString, ByteSlice};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::Write;
use std::mem;
use std::os::unix::ffi::OsStrExt;

#[derive(Debug)]
enum Type {
    U8,
    Bool,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F64,
    String,
    ObjectPath,
    Signature,
    Variant,
    Fd,
    Array(Box<Type>),
    DictEntry(Box<Type>, Box<Type>),
    Struct(Vec<Type>),
}

#[derive(Debug)]
struct Field {
    name: BString,
    ty: Type,
}

#[derive(Debug)]
struct Function {
    name: BString,
    in_fields: Vec<Field>,
    out_fields: Vec<Field>,
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<Vec<Function>> {
        let mut res = vec![];
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            match ty.as_bytes() {
                b"fn" => res.push(self.parse_fn()?.val),
                _ => bail!("In line {}: Unexpected entry {:?}", line, ty),
            }
        }
        Ok(res)
    }

    fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    fn not_eof(&self) -> Result<()> {
        if self.eof() {
            bail!("Unexpected eof");
        }
        Ok(())
    }

    fn parse_fn(&mut self) -> Result<Lined<Function>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            let (_, body) = self.expect_tree(TreeDelim::Paren)?;
            let mut parser = Parser {
                pos: 0,
                tokens: body,
            };
            let mut in_fields = vec![];
            while !parser.eof() {
                in_fields.push(parser.parse_field()?);
            }
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser {
                pos: 0,
                tokens: body,
            };
            let mut out_fields = vec![];
            while !parser.eof() {
                out_fields.push(parser.parse_field()?);
            }
            Ok(Lined {
                line,
                val: Function {
                    name: name.to_owned(),
                    in_fields,
                    out_fields,
                },
            })
        })();
        res.with_context(|| format!("While parsing message starting at line {}", line))
    }

    fn parse_field(&mut self) -> Result<Field> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Colon)?;
            let ty = self.parse_type()?;
            if !self.eof() {
                self.expect_symbol(Symbol::Comma)?;
            }
            Ok(Field {
                name: name.to_owned(),
                ty,
            })
        })();
        res.with_context(|| format!("While parsing field starting at line {}", line))
    }

    fn expect_ident(&mut self) -> Result<(u32, &'a BStr)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Ident(id) => Ok((token.line, *id)),
            k => bail!(
                "In line {}: Expected identifier, found {}",
                token.line,
                k.name()
            ),
        }
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<()> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Symbol(s) if *s == symbol => Ok(()),
            k => bail!(
                "In line {}: Expected {}, found {}",
                token.line,
                symbol.name(),
                k.name()
            ),
        }
    }

    fn expect_tree_(&mut self) -> Result<(u32, TreeDelim, &'a [Token<'a>])> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Tree { delim, body } => Ok((token.line, *delim, body)),
            k => bail!("In line {}: Expected tree, found {}", token.line, k.name()),
        }
    }

    fn expect_tree(&mut self, exp_delim: TreeDelim) -> Result<(u32, &'a [Token<'a>])> {
        let (line, delim, tokens) = self.expect_tree_()?;
        if delim == exp_delim {
            Ok((line, tokens))
        } else {
            bail!(
                "In line {}: Expected {:?}-delimited tree, found {:?}-delimited tree",
                line,
                exp_delim,
                delim.opening()
            )
        }
    }

    fn parse_type(&mut self) -> Result<Type> {
        self.not_eof()?;
        let (_, ty) = self.expect_ident()?;
        let ty = match ty.as_bytes() {
            b"u8" => Type::U8,
            b"bool" => Type::Bool,
            b"i16" => Type::I16,
            b"u16" => Type::U16,
            b"i32" => Type::I32,
            b"u32" => Type::U32,
            b"i64" => Type::I64,
            b"u64" => Type::U64,
            b"f64" => Type::F64,
            b"string" => Type::String,
            b"object_path" => Type::ObjectPath,
            b"signature" => Type::Signature,
            b"variant" => Type::Variant,
            b"fd" => Type::Fd,
            b"array" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser {
                    pos: 0,
                    tokens: body,
                };
                let ty: Result<_> = (|| {
                    let ty = parser.parse_type()?;
                    if !parser.eof() {
                        bail!("Trailing tokens in element type");
                    }
                    Ok(ty)
                })();
                let ty =
                    ty.with_context(|| format!("While parsing array starting in line {}", line))?;
                Type::Array(Box::new(ty))
            }
            b"dict" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser {
                    pos: 0,
                    tokens: body,
                };
                let ty: Result<_> = (|| {
                    let key = parser.parse_type()?;
                    parser.expect_symbol(Symbol::Comma)?;
                    let val = parser.parse_type()?;
                    Ok((key, val))
                })();
                let ty =
                    ty.with_context(|| format!("While parsing dict starting in line {}", line))?;
                Type::DictEntry(Box::new(ty.0), Box::new(ty.1))
            }
            b"struct" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser {
                    pos: 0,
                    tokens: body,
                };
                let mut fields = vec![];
                while !parser.eof() {
                    let ty: Result<_> = (|| {
                        let ty = parser.parse_type()?;
                        if !parser.eof() {
                            parser.expect_symbol(Symbol::Comma)?;
                        }
                        Ok(ty)
                    })();
                    let ty = ty.with_context(|| {
                        format!("While parsing struct starting in line {}", line)
                    })?;
                    fields.push(ty);
                }
                Type::Struct(fields)
            }
            _ => bail!("Unknown type {}", ty),
        };
        Ok(ty)
    }
}

fn parse_functions(s: &[u8]) -> Result<Vec<Function>> {
    let tokens = tokenize(s)?;
    let mut parser = Parser {
        pos: 0,
        tokens: &tokens,
    };
    parser.parse()
}

fn to_snake(s: &BStr) -> BString {
    let mut last_was_lowercase = false;
    let mut res = vec![];
    for mut b in s.as_bytes().iter().copied() {
        if b.is_ascii_uppercase() {
            if last_was_lowercase {
                res.push(b'_');
                last_was_lowercase = false;
            }
            b = b.to_ascii_lowercase();
        } else {
            last_was_lowercase = true;
        }
        res.push(b);
    }
    res.into()
}

fn needs_lifetime(ty: &Type) -> bool {
    match ty {
        Type::U8 => false,
        Type::Bool => false,
        Type::I16 => false,
        Type::U16 => false,
        Type::I32 => false,
        Type::U32 => false,
        Type::I64 => false,
        Type::U64 => false,
        Type::F64 => false,
        Type::String => true,
        Type::ObjectPath => true,
        Type::Signature => true,
        Type::Variant => true,
        Type::Fd => false,
        Type::Array(_) => true,
        Type::DictEntry(k, v) => needs_lifetime(k) || needs_lifetime(v),
        Type::Struct(fs) => fs.iter().any(needs_lifetime),
    }
}

fn write_signature<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    let c = match ty {
        Type::U8 => 'y',
        Type::Bool => 'b',
        Type::I16 => 'n',
        Type::U16 => 'q',
        Type::I32 => 'i',
        Type::U32 => 'u',
        Type::I64 => 'x',
        Type::U64 => 't',
        Type::F64 => 'd',
        Type::String => 's',
        Type::ObjectPath => 'o',
        Type::Signature => 'g',
        Type::Variant => 'v',
        Type::Fd => 'h',
        Type::Array(e) => {
            write!(f, "a")?;
            write_signature(f, e)?;
            return Ok(());
        }
        Type::DictEntry(k, v) => {
            write!(f, "{{")?;
            write_signature(f, k)?;
            write_signature(f, v)?;
            write!(f, "}}")?;
            return Ok(());
        }
        Type::Struct(fs) => {
            write!(f, "(")?;
            for fs in fs {
                write_signature(f, fs)?;
            }
            write!(f, ")")?;
            return Ok(());
        }
    };
    write!(f, "{}", c)?;
    Ok(())
}

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    let ty = match ty {
        Type::U8 => "u8",
        Type::Bool => "Bool",
        Type::I16 => "i16",
        Type::U16 => "u16",
        Type::I32 => "i32",
        Type::U32 => "u32",
        Type::I64 => "AlignedI64",
        Type::U64 => "AlignedU64",
        Type::F64 => "AlignedF64",
        Type::String => "Cow<'a, str>",
        Type::ObjectPath => "ObjectPath<'a>",
        Type::Signature => "Signature<'a>",
        Type::Variant => "Variant<'a>",
        Type::Fd => "Rc<OwnedFd>",
        Type::Array(e) => {
            write!(f, "Cow<'a, [")?;
            write_type(f, &e)?;
            write!(f, ">")?;
            return Ok(());
        }
        Type::DictEntry(k, v) => {
            write!(f, "DictEntry<")?;
            write_type(f, &k)?;
            write!(f, ", ")?;
            write_type(f, &v)?;
            write!(f, ">")?;
            return Ok(());
        }
        Type::Struct(fs) => {
            write!(f, "(")?;
            for (idx, fs) in fs.iter().enumerate() {
                if idx > 0 {
                    write!(f, ", ")?;
                }
                write_type(f, &fs)?;
            }
            write!(f, ")")?;
            return Ok(());
        }
    };
    write!(f, "{}", ty)?;
    Ok(())
}

fn write_message<W: Write>(
    f: &mut W,
    el: &Element,
    fun: &Function,
    name: &str,
    indent: &str,
    fields: &[Field],
    reply_name: Option<&str>,
    reply_has_lt: bool,
) -> Result<()> {
    let needs_lt = fields.iter().any(|f| needs_lifetime(&f.ty));
    let lt = if needs_lt { "<'a>" } else { "" };
    writeln!(f)?;
    if fields.is_empty() {
        writeln!(f, "{}pub struct {}{};", indent, name, lt)?;
    } else {
        writeln!(f, "{}pub struct {}{} {{", indent, name, lt)?;
        for field in fields {
            write!(f, "{}    pub {}: ", indent, field.name)?;
            write_type(f, &field.ty)?;
            writeln!(f, ",")?;
        }
        writeln!(f, "{}}}", indent)?;
    }
    writeln!(f)?;
    writeln!(f, "{}impl<'a> Message<'a> for {}{} {{", indent, name, lt)?;
    write!(f, "{}    const SIGNATURE: &'static str = \"", indent)?;
    for field in fields {
        write_signature(f, &field.ty)?;
    }
    writeln!(f, "\";")?;
    writeln!(
        f,
        "{}    const INTERFACE: &'static str = \"{}\";",
        indent, el.interface
    )?;
    writeln!(
        f,
        "{}    const MEMBER: &'static str = \"{}\";",
        indent, fun.name
    )?;
    writeln!(f)?;
    writeln!(f, "{}    fn marshal(&self, fmt: &mut Formatter) {{", indent)?;
    if fields.is_empty() {
        writeln!(f, "{}        let _ = fmt;", indent)?;
    }
    for field in fields {
        writeln!(f, "{}        fmt.marshal(&self.{});", indent, field.name)?;
    }
    writeln!(f, "{}    }}", indent)?;
    writeln!(f)?;
    writeln!(
        f,
        "{}    fn unmarshal(parser: &mut Parser<'a>) -> Result<Self, DbusError> {{",
        indent
    )?;
    if fields.is_empty() {
        writeln!(f, "{}        let _ = parser;", indent)?;
    }
    writeln!(f, "{}        Ok(Self {{", indent)?;
    for field in fields {
        writeln!(
            f,
            "{}            {}: parser.unmarshal()?,",
            indent, field.name
        )?;
    }
    writeln!(f, "{}        }})", indent)?;
    writeln!(f, "{}    }}", indent)?;
    writeln!(f)?;
    writeln!(f, "{}    fn num_fds(&self) -> u32 {{", indent)?;
    if fields.is_empty() {
        writeln!(f, "{}        0", indent)?;
    } else {
        writeln!(f, "{}        let mut res = 0;", indent)?;
        for field in fields {
            writeln!(f, "{}        res += self.{}.num_fds();", indent, field.name)?;
        }
        writeln!(f, "{}        res", indent)?;
    }
    writeln!(f, "{}    }}", indent)?;
    writeln!(f, "{}}}", indent)?;
    if let Some(rn) = reply_name {
        let reply_lt = if reply_has_lt { "<'b>" } else { "" };
        writeln!(f)?;
        writeln!(f, "{}impl<'a> MethodCall<'a> for {}{} {{", indent, name, lt)?;
        writeln!(f, "{}    type Reply<'b> = {}{};", indent, rn, reply_lt)?;
        writeln!(f, "{}}}", indent)?;
    }
    Ok(())
}

fn write_interface<W: Write>(f: &mut W, element: &Element, indent: &str) -> Result<()> {
    for fun in &element.functions {
        let in_name = format!("{}Call", fun.name);
        let out_name = format!("{}Reply", fun.name);
        let reply_has_lt = fun.out_fields.iter().any(|f| needs_lifetime(&f.ty));
        write_message(
            f,
            element,
            fun,
            &in_name,
            indent,
            &fun.in_fields,
            Some(&out_name),
            reply_has_lt,
        )?;
        write_message(
            f,
            element,
            fun,
            &out_name,
            indent,
            &fun.out_fields,
            None,
            false,
        )?;
    }
    Ok(())
}

fn write_module<W: Write>(f: &mut W, element: Element, indent: &str) -> Result<()> {
    let mut children: Vec<_> = element.children.into_iter().map(|v| v.1).collect();
    children.sort_by(|c1, c2| c1.name.cmp(&c2.name));
    for child in children {
        write_element(f, child, indent)?;
    }
    Ok(())
}

fn write_element<W: Write>(f: &mut W, element: Element, indent: &str) -> Result<()> {
    writeln!(f)?;
    writeln!(f, "{}pub mod {} {{", indent, element.name)?;
    writeln!(f, "{}    use crate::dbus::prelude::*;", indent)?;
    {
        let indent = format!("{}    ", indent);
        write_interface(f, &element, &indent)?;
        write_module(f, element, &indent)?;
    }
    writeln!(f, "{}}}", indent)?;
    Ok(())
}

struct Element {
    name: BString,
    interface: BString,
    children: HashMap<BString, Element>,
    functions: Vec<Function>,
}

fn collect_interfaces() -> Result<Element> {
    let mut root = Element {
        name: Default::default(),
        interface: Default::default(),
        children: Default::default(),
        functions: vec![],
    };
    let mut files = vec![];
    for file in std::fs::read_dir("wire-dbus")? {
        files.push(file?);
    }
    for file in files {
        let file_name = file.file_name();
        let file_name = file_name.as_bytes().as_bstr();
        println!("cargo:rerun-if-changed=wire-dbus/{}", file_name);
        let mut interface = file_name
            .rsplitn_str(2, ".")
            .skip(1)
            .next()
            .unwrap()
            .as_bstr()
            .to_owned();
        let mut components: Vec<_> = file_name.split_str(".").collect();
        components.pop();
        let functions = (|| {
            let contents = std::fs::read(file.path())?;
            parse_functions(&contents)
        })();
        let mut functions =
            functions.with_context(|| format!("While parsing file {}", file.path().display()))?;
        let mut target = &mut root;
        for (i, comp) in components.iter().enumerate() {
            let comp = comp.as_bstr();
            if i + 1 < components.len() {
                target = match target.children.entry(comp.to_owned()) {
                    Entry::Occupied(o) => o.into_mut(),
                    Entry::Vacant(v) => v.insert(Element {
                        name: comp.to_owned(),
                        interface: Default::default(),
                        children: HashMap::new(),
                        functions: Vec::new(),
                    }),
                };
            } else {
                target.children.insert(
                    comp.to_owned(),
                    Element {
                        name: to_snake(comp),
                        interface: mem::take(&mut interface),
                        children: Default::default(),
                        functions: mem::take(&mut functions),
                    },
                );
            }
        }
    }
    Ok(root)
}

pub fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=wire-dbus");

    let mut f = open("wire_dbus.rs")?;
    for (_, child) in collect_interfaces()?.children {
        write_element(&mut f, child, "")?;
    }
    Ok(())
}

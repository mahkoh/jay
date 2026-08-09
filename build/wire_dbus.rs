use crate::indent::Indent;
use crate::open;
use crate::tokens::Lined;
use crate::tokens::Symbol;
use crate::tokens::Token;
use crate::tokens::TokenKind;
use crate::tokens::TreeDelim;
use crate::tokens::tokenize;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use bstr::BStr;
use bstr::BString;
use bstr::ByteSlice;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::Write;
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
    name: String,
    ty: Type,
}

#[derive(Debug)]
struct Function {
    name: String,
    in_fields: Vec<Field>,
    out_fields: Vec<Field>,
}

#[derive(Debug)]
struct Property {
    name: String,
    ty: Type,
}

#[derive(Debug)]
struct Signal {
    name: String,
    fields: Vec<Field>,
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<Component> {
        let mut res = Component {
            functions: vec![],
            properties: vec![],
            signals: vec![],
        };
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            match ty.as_bytes() {
                b"fn" => res.functions.push(self.parse_fn()?.val),
                b"prop" => res.properties.push(self.parse_prop()?.val),
                b"sig" => res.signals.push(self.parse_signal()?.val),
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

    fn parse_prop(&mut self) -> Result<Lined<Property>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Equals)?;
            let ty = self.parse_type()?;
            Ok(Lined {
                line,
                val: Property {
                    name: name.to_owned(),
                    ty,
                },
            })
        })();
        res.with_context(|| format!("While parsing property starting at line {}", line))
    }

    fn parse_signal(&mut self) -> Result<Lined<Signal>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser {
                pos: 0,
                tokens: body,
            };
            let mut fields = vec![];
            while !parser.eof() {
                fields.push(parser.parse_field()?);
            }
            Ok(Lined {
                line,
                val: Signal {
                    name: name.to_owned(),
                    fields,
                },
            })
        })();
        res.with_context(|| format!("While parsing signal starting at line {}", line))
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
        res.with_context(|| format!("While parsing function starting at line {}", line))
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

    fn expect_ident(&mut self) -> Result<(u32, &'a str)> {
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

fn parse_component(s: &[u8]) -> Result<Component> {
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
    define_w!(f, w, wl);
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
            w!("a");
            write_signature(f, e)?;
            return Ok(());
        }
        Type::DictEntry(k, v) => {
            w!("{{");
            write_signature(f, k)?;
            write_signature(f, v)?;
            w!("}}");
            return Ok(());
        }
        Type::Struct(fs) => {
            w!("(");
            for fs in fs {
                write_signature(f, fs)?;
            }
            w!(")");
            return Ok(());
        }
    };
    w!("{}", c);
    Ok(())
}

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    write_type2(f, "'a", ty)
}

fn write_type2<W: Write>(f: &mut W, lt: &str, ty: &Type) -> Result<()> {
    define_w!(f, w, wl);
    let ty = match ty {
        Type::U8 => "u8",
        Type::Bool => "Bool",
        Type::I16 => "i16",
        Type::U16 => "u16",
        Type::I32 => "i32",
        Type::U32 => "u32",
        Type::I64 => "i64",
        Type::U64 => "u64",
        Type::F64 => "f64",
        Type::String => {
            w!("Cow<{}, str>", lt);
            return Ok(());
        }
        Type::ObjectPath => {
            w!("ObjectPath<{}>", lt);
            return Ok(());
        }
        Type::Signature => {
            w!("Signature<{}>", lt);
            return Ok(());
        }
        Type::Variant => {
            w!("Variant<{}>", lt);
            return Ok(());
        }
        Type::Fd => "Rc<OwnedFd>",
        Type::Array(e) => {
            w!("Cow<{}, [", lt);
            write_type2(f, lt, e)?;
            w!("]>");
            return Ok(());
        }
        Type::DictEntry(k, v) => {
            w!("DictEntry<");
            write_type2(f, lt, k)?;
            w!(", ");
            write_type2(f, lt, v)?;
            w!(">");
            return Ok(());
        }
        Type::Struct(fs) => {
            w!("(");
            for (idx, fs) in fs.iter().enumerate() {
                if idx > 0 {
                    w!(", ");
                }
                write_type2(f, lt, fs)?;
            }
            w!(")");
            return Ok(());
        }
    };
    w!("{}", ty);
    Ok(())
}

fn write_message<W: Write>(
    f: &mut W,
    xn: &Indent,
    el: &Element,
    msg_name: &str,
    name: &str,
    fields: &[Field],
    reply_name: Option<&str>,
    reply_has_lt: bool,
) -> Result<()> {
    define_w!(f, w, wl);
    let needs_lt = fields.iter().any(|f| needs_lifetime(&f.ty));
    let lt = if needs_lt { "<'a>" } else { "" };
    let ltb = if needs_lt { "<'b>" } else { "" };
    wl!();
    wl!("{xn}#[derive(Debug)]");
    if fields.is_empty() {
        wl!("{xn}pub struct {}{};", name, lt);
    } else {
        wl!("{xn}pub struct {}{} {{", name, lt);
        {
            push_xn!(xn);
            for field in fields {
                w!("{xn}pub {}: ", field.name);
                write_type(f, &field.ty)?;
                wl!(",");
            }
        }
        wl!("{xn}}}");
    }
    wl!();
    wl!("{xn}unsafe impl<'a> Message<'a> for {}{} {{", name, lt);
    {
        push_xn!(xn);
        w!("{xn}const SIGNATURE: &'static str = \"");
        for field in fields {
            write_signature(f, &field.ty)?;
        }
        wl!("\";");
        wl!("{xn}const INTERFACE: &'static str = \"{}\";", el.interface);
        wl!("{xn}const MEMBER: &'static str = \"{}\";", msg_name,);
        wl!("{xn}type Generic<'b> = {}{};", name, ltb,);
        wl!();
        wl!("{xn}fn marshal(&self, fmt: &mut Formatter) {{");
        {
            push_xn!(xn);
            if fields.is_empty() {
                wl!("{xn}let _ = fmt;");
            }
            for field in fields {
                wl!("{xn}fmt.marshal(&self.{});", field.name);
            }
        }
        wl!("{xn}}}");
        wl!();
        wl!("{xn}fn unmarshal(parser: &mut Parser<'a>) -> Result<Self, DbusError> {{",);
        {
            push_xn!(xn);
            if fields.is_empty() {
                wl!("{xn}let _ = parser;");
            }
            wl!("{xn}Ok(Self {{");
            {
                push_xn!(xn);
                for field in fields {
                    wl!("{xn}{}: parser.unmarshal()?,", field.name);
                }
            }
            wl!("{xn}}})");
        }
        wl!("{xn}}}");
        wl!();
        wl!("{xn}fn num_fds(&self) -> u32 {{");
        {
            push_xn!(xn);
            if fields.is_empty() {
                wl!("{xn}0");
            } else {
                wl!("{xn}let mut res = 0;");
                for field in fields {
                    wl!("{xn}res += self.{}.num_fds();", field.name);
                }
                wl!("{xn}res");
            }
        }
        wl!("{xn}}}");
    }
    wl!("{xn}}}");
    if let Some(rn) = reply_name {
        let reply_lt = if reply_has_lt { "<'static>" } else { "" };
        wl!();
        wl!("{xn}impl<'a> MethodCall<'a> for {}{} {{", name, lt);
        {
            push_xn!(xn);
            wl!("{xn}type Reply = {}{};", rn, reply_lt);
        }
        wl!("{xn}}}");
    }
    Ok(())
}

fn write_component<W: Write>(
    f: &mut W,
    xn: &Indent,
    element: &Element,
    component: &Component,
) -> Result<()> {
    for fun in &component.functions {
        write_function(f, xn, element, fun)?;
    }
    for prop in &component.properties {
        write_property(f, xn, element, prop)?;
    }
    for sig in &component.signals {
        write_signal(f, xn, element, sig)?;
    }
    Ok(())
}

fn write_property<W: Write>(
    f: &mut W,
    xn: &Indent,
    el: &Element,
    property: &Property,
) -> Result<()> {
    define_w!(f, w, wl);
    wl!();
    wl!("{xn}pub struct {};", property.name);
    wl!();
    wl!("{xn}impl Property for {} {{", property.name);
    {
        push_xn!(xn);
        wl!("{xn}const INTERFACE: &'static str = \"{}\";", el.interface);
        wl!("{xn}const PROPERTY: &'static str = \"{}\";", property.name,);
        w!("{xn}type Type = ");
        write_type2(f, "'static", &property.ty)?;
        wl!(";");
    }
    wl!("{xn}}}");
    Ok(())
}

fn write_signal<W: Write>(f: &mut W, xn: &Indent, element: &Element, sig: &Signal) -> Result<()> {
    define_w!(f, w, wl);
    let name = format!("{}", sig.name);
    write_message(f, xn, element, &sig.name, &name, &sig.fields, None, false)?;
    let has_lt = sig.fields.iter().any(|f| needs_lifetime(&f.ty));
    let lt = if has_lt { "<'a>" } else { "" };
    wl!();
    wl!("{xn}impl<'a> Signal<'a> for {}{} {{ }}", name, lt);
    Ok(())
}

fn write_function<W: Write>(
    f: &mut W,
    xn: &Indent,
    element: &Element,
    fun: &Function,
) -> Result<()> {
    let in_name = format!("{}", fun.name);
    let out_name = format!("{}Reply", fun.name);
    let reply_has_lt = fun.out_fields.iter().any(|f| needs_lifetime(&f.ty));
    write_message(
        f,
        xn,
        element,
        &fun.name,
        &in_name,
        &fun.in_fields,
        Some(&out_name),
        reply_has_lt,
    )?;
    write_message(
        f,
        xn,
        element,
        &fun.name,
        &out_name,
        &fun.out_fields,
        None,
        false,
    )?;
    Ok(())
}

fn write_module<W: Write>(f: &mut W, xn: &Indent, element: Element) -> Result<()> {
    let mut children: Vec<_> = element.children.into_iter().map(|v| v.1).collect();
    children.sort_by(|c1, c2| c1.name.cmp(&c2.name));
    for child in children {
        write_element(f, xn, child)?;
    }
    Ok(())
}

fn write_element<W: Write>(f: &mut W, xn: &Indent, element: Element) -> Result<()> {
    define_w!(f, w, wl);
    let name = if element.name == "impl" {
        "impl_".as_bytes().as_bstr()
    } else {
        element.name.as_bstr()
    };
    wl!();
    wl!("{xn}pub mod {} {{", name);
    {
        push_xn!(xn);
        wl!("{xn}use crate::dbus::prelude::*;");
        for component in &element.components {
            write_component(f, xn, &element, component)?;
        }
        write_module(f, xn, element)?;
    }
    wl!("{xn}}}");
    Ok(())
}

#[derive(Debug)]
struct Component {
    functions: Vec<Function>,
    properties: Vec<Property>,
    signals: Vec<Signal>,
}

#[derive(Debug)]
struct Element {
    name: BString,
    interface: BString,
    children: HashMap<BString, Element>,
    components: Vec<Component>,
}

fn collect_interfaces() -> Result<Element> {
    let mut root = Element {
        name: Default::default(),
        interface: Default::default(),
        children: Default::default(),
        components: vec![],
    };
    let mut files = vec![];
    for file in std::fs::read_dir("wire-dbus")? {
        files.push(file?);
    }
    for file in files {
        let file_name = file.file_name();
        let file_name = file_name.as_bytes().as_bstr();
        println!("cargo:rerun-if-changed=wire-dbus/{}", file_name);
        let interface = file_name
            .rsplitn_str(2, ".")
            .skip(1)
            .next()
            .unwrap()
            .as_bstr()
            .to_owned();
        let mut components: Vec<_> = file_name.split_str(".").collect();
        components.pop();
        let component = (|| {
            let contents = std::fs::read(file.path())?;
            parse_component(&contents)
        })();
        let component =
            component.with_context(|| format!("While parsing file {}", file.path().display()))?;
        let mut target = &mut root;
        for comp in components.iter() {
            let comp = to_snake(comp.as_bstr());
            target = match target.children.entry(comp.to_owned()) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(Element {
                    name: comp.to_owned(),
                    interface: Default::default(),
                    children: HashMap::new(),
                    components: vec![],
                }),
            };
        }
        target.interface = interface;
        target.components.push(component);
    }
    Ok(root)
}

pub fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=wire-dbus");

    let mut f = open("wire_dbus.rs")?;
    let mut children: Vec<_> = collect_interfaces()?
        .children
        .into_iter()
        .map(|v| v.1)
        .collect();
    children.sort_by(|c1, c2| c1.name.cmp(&c2.name));
    for child in children {
        write_element(&mut f, &Default::default(), child)?;
    }
    Ok(())
}

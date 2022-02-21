use crate::open;
use anyhow::{bail, Context, Result};
use bstr::{BStr, BString, ByteSlice};
use std::fs::DirEntry;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TreeDelim {
    Paren,
    Brace,
}

impl TreeDelim {
    fn opening(self) -> u8 {
        match self {
            TreeDelim::Paren => b'(',
            TreeDelim::Brace => b'{',
        }
    }

    fn closing(self) -> u8 {
        match self {
            TreeDelim::Paren => b')',
            TreeDelim::Brace => b'}',
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Symbol {
    Comma,
    Colon,
    Equals,
}

impl Symbol {
    fn name(self) -> &'static str {
        match self {
            Symbol::Comma => "','",
            Symbol::Colon => "':'",
            Symbol::Equals => "'='",
        }
    }
}

#[derive(Debug)]
struct Token<'a> {
    line: u32,
    kind: TokenKind<'a>,
}

#[derive(Debug)]
enum TokenKind<'a> {
    Ident(&'a BStr),
    Num(u32),
    Tree {
        delim: TreeDelim,
        body: Vec<Token<'a>>,
    },
    Symbol(Symbol),
}

impl TokenKind<'_> {
    fn name(&self) -> &str {
        match self {
            TokenKind::Ident(_) => "identifier",
            TokenKind::Num(_) => "number",
            TokenKind::Tree { delim, .. } => match delim {
                TreeDelim::Paren => "'('-tree",
                TreeDelim::Brace => "'{'-tree",
            },
            TokenKind::Symbol(s) => s.name(),
        }
    }
}

#[derive(Copy, Clone)]
struct Cursor<'a> {
    pos: usize,
    s: &'a [u8],
}

impl Cursor<'_> {
    fn eof(&self) -> bool {
        self.pos >= self.s.len()
    }
}

fn tokenize<'a>(s: &'a [u8]) -> Result<Vec<Token<'a>>> {
    let mut tnz = Tokenizer {
        line: 1,
        cursor: Cursor { pos: 0, s },
        delim: None,
        res: vec![],
    };
    tnz.tokenize()?;
    Ok(tnz.res)
}

struct Tokenizer<'a> {
    line: u32,
    cursor: Cursor<'a>,
    delim: Option<TreeDelim>,
    res: Vec<Token<'a>>,
}

impl<'a> Tokenizer<'a> {
    fn tokenize_one(&mut self) -> Result<bool> {
        let c = &mut self.cursor;
        while !c.eof() {
            let b = c.s[c.pos];
            if matches!(b, b' ' | b'\n' | b'#') {
                c.pos += 1;
                if b == b'\n' {
                    self.line += 1;
                } else if b == b'#' {
                    while !c.eof() {
                        c.pos += 1;
                        if c.s[c.pos - 1] == b'\n' {
                            self.line += 1;
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
        if c.eof() {
            if self.delim.is_some() {
                bail!("Unexpected eof");
            }
            return Ok(false);
        }
        let line = self.line;
        let b = c.s[c.pos];
        let b_pos = c.pos;
        c.pos += 1;
        let kind = match b {
            b'a'..=b'z' => {
                while !c.eof() && matches!(c.s[c.pos], b'a'..=b'z' | b'_' | b'0'..=b'9') {
                    c.pos += 1;
                }
                TokenKind::Ident(c.s[b_pos..c.pos].as_bstr())
            }
            b'0'..=b'9' => {
                c.pos -= 1;
                let mut num = 0;
                while !c.eof() && matches!(c.s[c.pos], b'0'..=b'9') {
                    num = num * 10 + (c.s[c.pos] - b'0') as u32;
                    c.pos += 1;
                }
                TokenKind::Num(num)
            }
            b',' => TokenKind::Symbol(Symbol::Comma),
            b'=' => TokenKind::Symbol(Symbol::Equals),
            b':' => TokenKind::Symbol(Symbol::Colon),
            b'(' => self.tokenize_tree(TreeDelim::Paren)?,
            b'{' => self.tokenize_tree(TreeDelim::Brace)?,
            c @ (b')' | b'}') => {
                if self.delim.map(|d| d.closing()) != Some(c) {
                    bail!("Unexpected '{}' in line {}", c, self.line);
                }
                return Ok(false);
            }
            _ => bail!("Unexpected byte {:?} in line {}", b as char, self.line),
        };
        self.res.push(Token { line, kind });
        Ok(true)
    }

    fn tokenize(&mut self) -> Result<()> {
        while self.tokenize_one()? {
            // nothing
        }
        Ok(())
    }

    fn tokenize_tree(&mut self, delim: TreeDelim) -> Result<TokenKind<'a>> {
        let mut tnz = Tokenizer {
            line: self.line,
            cursor: self.cursor,
            delim: Some(delim),
            res: vec![],
        };
        tnz.tokenize().with_context(|| {
            format!(
                "While tokenizing {:?} block starting in line {}",
                delim.opening() as char,
                self.line
            )
        })?;
        self.cursor.pos = tnz.cursor.pos;
        self.line = tnz.line;
        Ok(TokenKind::Tree {
            delim,
            body: tnz.res,
        })
    }
}

#[derive(Debug)]
struct Lined<T> {
    #[allow(dead_code)]
    line: u32,
    val: T,
}

#[derive(Debug)]
enum Type {
    Id(BString),
    U32,
    I32,
    Str,
    OptStr,
    BStr,
    Fixed,
    Fd,
    Array(Box<Type>),
    Pod(BString),
}

#[derive(Debug)]
struct Field {
    name: BString,
    ty: Lined<Type>,
}

#[derive(Debug)]
struct Message {
    name: BString,
    camel_name: BString,
    id: Lined<u32>,
    fields: Vec<Lined<Field>>,
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<Vec<Lined<Message>>> {
        let mut res = vec![];
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            match ty.as_bytes() {
                b"msg" => res.push(self.parse_message()?),
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

    fn yes_eof(&self) -> Result<()> {
        if !self.eof() {
            bail!(
                "Unexpected trailing tokens in line {}",
                self.tokens[self.pos].line
            );
        }
        Ok(())
    }

    fn parse_message(&mut self) -> Result<Lined<Message>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Equals)?;
            let (num_line, val) = self.expect_number()?;
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
                val: Message {
                    name: name.to_owned(),
                    camel_name: to_camel(name),
                    id: Lined {
                        line: num_line,
                        val,
                    },
                    fields,
                },
            })
        })();
        res.with_context(|| format!("While parsing message starting at line {}", line))
    }

    fn parse_field(&mut self) -> Result<Lined<Field>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Colon)?;
            let ty = self.parse_type()?;
            if !self.eof() {
                self.expect_symbol(Symbol::Comma)?;
            }
            Ok(Lined {
                line,
                val: Field {
                    name: name.to_owned(),
                    ty,
                },
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

    fn expect_number(&mut self) -> Result<(u32, u32)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Num(n) => Ok((token.line, *n)),
            k => bail!(
                "In line {}: Expected number, found {}",
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

    fn parse_rust_path(&mut self) -> Result<Lined<BString>> {
        let mut path = Vec::new();
        let mut line = None;
        loop {
            self.not_eof()?;
            let (l, id) = self.expect_ident()?;
            if line.is_none() {
                line = Some(l);
            }
            path.extend_from_slice(id.as_bytes());
            if self.eof() {
                break;
            }
            self.expect_symbol(Symbol::Colon)?;
            self.expect_symbol(Symbol::Colon)?;
            path.extend_from_slice(b"::");
        }
        Ok(Lined {
            line: line.unwrap(),
            val: path.into(),
        })
    }

    fn parse_type(&mut self) -> Result<Lined<Type>> {
        self.not_eof()?;
        let (line, ty) = self.expect_ident()?;
        let ty = match ty.as_bytes() {
            b"pod" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser {
                    pos: 0,
                    tokens: body,
                };
                let ty = parser.parse_rust_path().with_context(|| {
                    format!("While parsing pod element type starting in line {}", line)
                })?;
                Type::Pod(ty.val)
            }
            b"u32" => Type::U32,
            b"i32" => Type::I32,
            b"str" => Type::Str,
            b"optstr" => Type::OptStr,
            b"bstr" => Type::BStr,
            b"fixed" => Type::Fixed,
            b"fd" => Type::Fd,
            b"array" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let ty: Result<_> = (|| {
                    let mut parser = Parser {
                        pos: 0,
                        tokens: body,
                    };
                    let ty = parser.parse_type()?;
                    parser.yes_eof()?;
                    match &ty.val {
                        Type::Id(_) => {}
                        Type::U32 => {}
                        Type::I32 => {}
                        Type::Fixed => {}
                        Type::Pod(..) => {}
                        _ => {
                            bail!("Only numerical and pod types can be array elements");
                        }
                    }
                    Ok(ty)
                })();
                let ty = ty.with_context(|| {
                    format!("While parsing array element type starting in line {}", line)
                })?;
                Type::Array(Box::new(ty.val))
            }
            b"id" => {
                let (_, body) = self.expect_tree(TreeDelim::Paren)?;
                let ident: Result<_> = (|| {
                    let mut parser = Parser {
                        pos: 0,
                        tokens: body,
                    };
                    let id = parser.expect_ident()?;
                    parser.yes_eof()?;
                    Ok(id)
                })();
                let (_, ident) = ident.with_context(|| {
                    format!("While parsing identifier starting in line {}", line)
                })?;
                Type::Id(to_camel(ident))
            }
            _ => bail!("Unknown type {}", ty),
        };
        Ok(Lined { line, val: ty })
    }
}

fn parse_messages(s: &[u8]) -> Result<Vec<Lined<Message>>> {
    let tokens = tokenize(s)?;
    let mut parser = Parser {
        pos: 0,
        tokens: &tokens,
    };
    parser.parse()
}

fn to_camel(s: &BStr) -> BString {
    let mut last_was_underscore = true;
    let mut res = vec![];
    for mut b in s.as_bytes().iter().copied() {
        if b == b'_' {
            last_was_underscore = true;
        } else {
            if last_was_underscore {
                b = b.to_ascii_uppercase()
            }
            res.push(b);
            last_was_underscore = false;
        }
    }
    res.into()
}

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    match ty {
        Type::Id(id) => write!(f, "{}Id", id)?,
        Type::U32 => write!(f, "u32")?,
        Type::I32 => write!(f, "i32")?,
        Type::Str => write!(f, "&'a str")?,
        Type::OptStr => write!(f, "Option<&'a str>")?,
        Type::BStr => write!(f, "&'a BStr")?,
        Type::Fixed => write!(f, "Fixed")?,
        Type::Fd => write!(f, "Rc<OwnedFd>")?,
        Type::Array(n) => {
            write!(f, "&'a [")?;
            write_type(f, n)?;
            write!(f, "]")?;
        }
        Type::Pod(p) => f.write_all(p.as_bytes())?,
    }
    Ok(())
}

fn write_field<W: Write>(f: &mut W, field: &Field) -> Result<()> {
    write!(f, "        pub {}: ", field.name)?;
    write_type(f, &field.ty.val)?;
    writeln!(f, ",")?;
    Ok(())
}

fn write_message_type<W: Write>(
    f: &mut W,
    obj: &BStr,
    message: &Message,
    needs_lifetime: bool,
) -> Result<()> {
    let lifetime = if needs_lifetime { "<'a>" } else { "" };
    writeln!(f, "    pub struct {}{} {{", message.camel_name, lifetime)?;
    writeln!(f, "        pub self_id: {}Id,", obj)?;
    for field in &message.fields {
        write_field(f, &field.val)?;
    }
    writeln!(f, "    }}")?;
    writeln!(
        f,
        "    impl{} std::fmt::Debug for {}{} {{",
        lifetime, message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
    )?;
    write!(f, r#"            write!(fmt, "{}("#, message.name)?;
    for (i, field) in message.fields.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        let formatter = match &field.val.ty.val {
            Type::OptStr | Type::Str | Type::Fd | Type::Array(..) => "{:?}",
            _ => "{}",
        };
        write!(f, "{}: {}", field.val.name, formatter)?;
    }
    write!(f, r#")""#)?;
    for field in &message.fields {
        write!(f, ", self.{}", field.val.name)?;
    }
    writeln!(f, r")")?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_message<W: Write>(f: &mut W, obj: &BStr, message: &Message) -> Result<()> {
    let has_reference_type = message.fields.iter().any(|f| match &f.val.ty.val {
        Type::OptStr | Type::Str | Type::BStr | Type::Array(..) => true,
        _ => false,
    });
    let uppercase = message.name.to_ascii_uppercase();
    let uppercase = uppercase.as_bstr();
    writeln!(f)?;
    writeln!(f, "    pub const {}: u32 = {};", uppercase, message.id.val)?;
    write_message_type(f, obj, message, has_reference_type)?;
    let lifetime = if has_reference_type { "<'a>" } else { "" };
    let parser = if message.fields.len() > 0 {
        "parser"
    } else {
        "_parser"
    };
    writeln!(
        f,
        "    impl<'a> RequestParser<'a> for {}{} {{",
        message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        fn parse({}: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {{",
        parser
    )?;
    writeln!(f, "            Ok(Self {{")?;
    writeln!(f, "                self_id: {}Id::NONE,", obj)?;
    for field in &message.fields {
        let p = match &field.val.ty.val {
            Type::Id(_) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::OptStr => "optstr",
            Type::Str => "str",
            Type::Fixed => "fixed",
            Type::Fd => "fd",
            Type::BStr => "bstr",
            Type::Array(_) => "binary_array",
            Type::Pod(_) => "binary",
        };
        writeln!(f, "                {}: parser.{}()?,", field.val.name, p)?;
    }
    writeln!(f, "            }})")?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    writeln!(
        f,
        "    impl{} EventFormatter for {}{} {{",
        lifetime, message.camel_name, lifetime
    )?;
    writeln!(f, "        fn format(self, fmt: &mut MsgFormatter<'_>) {{")?;
    writeln!(f, "            fmt.header(self.self_id, {});", uppercase)?;
    fn write_fmt_expr<W: Write>(f: &mut W, prefix: &str, ty: &Type, access: &str) -> Result<()> {
        let p = match ty {
            Type::Id(_) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::OptStr => "optstr",
            Type::Str | Type::BStr => "string",
            Type::Fixed => "fixed",
            Type::Fd => "fd",
            Type::Array(..) => "binary",
            Type::Pod(..) => "binary",
        };
        let rf = match ty {
            Type::Pod(..) => "&",
            _ => "",
        };
        writeln!(f, "            {}fmt.{}({}{});", prefix, p, rf, access)?;
        Ok(())
    }
    for field in &message.fields {
        write_fmt_expr(
            f,
            "",
            &field.val.ty.val,
            &format!("self.{}", field.val.name),
        )?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "        fn id(&self) -> ObjectId {{")?;
    writeln!(f, "            self.self_id.into()")?;
    writeln!(f, "        }}")?;
    writeln!(f, "        fn interface(&self) -> Interface {{")?;
    writeln!(f, "            {}", obj)?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_file<W: Write>(f: &mut W, file: &DirEntry) -> Result<()> {
    let file_name = file.file_name();
    let file_name = file_name.as_bytes().as_bstr();
    println!("cargo:rerun-if-changed=wire/{}", file_name);
    let obj_name = file_name.split_str(".").next().unwrap().as_bstr();
    let camel_obj_name = to_camel(obj_name);
    writeln!(f)?;
    writeln!(f, "id!({}Id);", camel_obj_name)?;
    writeln!(f)?;
    writeln!(
        f,
        "pub const {}: Interface = Interface(\"{}\");",
        camel_obj_name, obj_name
    )?;
    let contents = std::fs::read(file.path())?;
    let messages = parse_messages(&contents)?;
    if messages.is_empty() {
        return Ok(());
    }
    writeln!(f)?;
    writeln!(f, "pub mod {} {{", obj_name)?;
    writeln!(f, "    use super::*;")?;
    for message in &messages {
        write_message(f, camel_obj_name.as_bstr(), &message.val)?;
    }
    writeln!(f, "}}")?;
    Ok(())
}

pub fn main() -> Result<()> {
    let mut f = open("wire.rs")?;
    writeln!(f, "use std::rc::Rc;")?;
    writeln!(f, "use uapi::OwnedFd;")?;
    writeln!(f, "use bstr::BStr;")?;
    writeln!(f, "use crate::fixed::Fixed;")?;
    writeln!(f, "use crate::client::{{EventFormatter, RequestParser}};")?;
    writeln!(f, "use crate::object::{{ObjectId, Interface}};")?;
    writeln!(
        f,
        "use crate::utils::buffd::{{MsgFormatter, MsgParser, MsgParserError}};"
    )?;
    println!("cargo:rerun-if-changed=wire");
    let mut files = vec![];
    for file in std::fs::read_dir("wire")? {
        files.push(file?);
    }
    files.sort_by_key(|f| f.file_name());
    for file in files {
        write_file(&mut f, &file)
            .with_context(|| format!("While processing {}", file.path().display()))?;
    }
    Ok(())
}

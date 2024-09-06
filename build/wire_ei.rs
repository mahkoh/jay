use {
    crate::{
        open,
        tokens::{tokenize, Symbol, Token, TokenKind, TreeDelim},
    },
    anyhow::{bail, Context, Result},
    std::{fs::DirEntry, io::Write, os::unix::ffi::OsStrExt},
};

#[derive(Debug)]
struct Lined<T> {
    #[expect(dead_code)]
    line: u32,
    val: T,
}

#[derive(Debug)]
enum Type {
    Id(String),
    U32,
    I32,
    U64,
    I64,
    F32,
    Str,
    OptStr,
    Fd,
}

#[derive(Debug)]
struct Field {
    name: String,
    ty: Lined<Type>,
}

#[derive(Debug)]
struct Message {
    name: String,
    camel_name: String,
    safe_name: String,
    id: u32,
    fields: Vec<Lined<Field>>,
    attribs: MessageAttribs,
    has_reference_type: bool,
}

#[derive(Debug, Default)]
struct MessageAttribs {
    since: Option<u32>,
    context: Option<&'static str>,
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

struct ParseResult {
    requests: Vec<Lined<Message>>,
    events: Vec<Lined<Message>>,
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<ParseResult> {
        let mut requests = vec![];
        let mut events = vec![];
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            let res = match ty.as_bytes() {
                b"request" => &mut requests,
                b"event" => &mut events,
                _ => bail!("In line {}: Unexpected entry {:?}", line, ty),
            };
            res.push(self.parse_message(res.len() as _)?);
        }
        Ok(ParseResult { requests, events })
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

    fn parse_message_attribs(&mut self, attribs: &mut MessageAttribs) -> Result<()> {
        let (_, tokens) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser { pos: 0, tokens };
        while !parser.eof() {
            let (line, name) = parser.expect_ident()?;
            match name {
                "since" => {
                    parser.expect_symbol(Symbol::Equals)?;
                    attribs.since = Some(parser.expect_number()?.1)
                }
                "receiver" => attribs.context = Some("Receiver"),
                "sender" => attribs.context = Some("Sender"),
                _ => bail!("In line {}: Unexpected attribute {}", line, name),
            }
            if !parser.eof() {
                parser.expect_symbol(Symbol::Comma)?;
            }
        }
        Ok(())
    }

    fn parse_message(&mut self, id: u32) -> Result<Lined<Message>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.not_eof()?;
            let mut attribs = MessageAttribs::default();
            if let TokenKind::Tree {
                delim: TreeDelim::Paren,
                ..
            } = self.tokens[self.pos].kind
            {
                self.parse_message_attribs(&mut attribs)?;
            }
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser {
                pos: 0,
                tokens: body,
            };
            let mut fields = vec![];
            while !parser.eof() {
                fields.push(parser.parse_field()?);
            }
            let has_reference_type = fields.iter().any(|f| match &f.val.ty.val {
                Type::OptStr | Type::Str => true,
                _ => false,
            });
            let safe_name = match name {
                "move" => "move_",
                _ => name,
            };
            Ok(Lined {
                line,
                val: Message {
                    name: name.to_owned(),
                    camel_name: to_camel(name),
                    safe_name: safe_name.to_string(),
                    id,
                    fields,
                    attribs,
                    has_reference_type,
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

    fn parse_type(&mut self) -> Result<Lined<Type>> {
        self.not_eof()?;
        let (line, ty) = self.expect_ident()?;
        let ty = match ty.as_bytes() {
            b"u32" => Type::U32,
            b"i32" => Type::I32,
            b"u64" => Type::U64,
            b"i64" => Type::I64,
            b"f32" => Type::F32,
            b"str" => Type::Str,
            b"optstr" => Type::OptStr,
            b"fd" => Type::Fd,
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

fn parse_messages(s: &[u8]) -> Result<ParseResult> {
    let tokens = tokenize(s)?;
    let mut parser = Parser {
        pos: 0,
        tokens: &tokens,
    };
    parser.parse()
}

fn to_camel(s: &str) -> String {
    let mut last_was_underscore = true;
    let mut res = String::new();
    for mut b in s.as_bytes().iter().copied() {
        if b == b'_' {
            last_was_underscore = true;
        } else {
            if last_was_underscore {
                b = b.to_ascii_uppercase()
            }
            res.push(b as char);
            last_was_underscore = false;
        }
    }
    res
}

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    let ty = match ty {
        Type::Id(id) => {
            write!(f, "{}Id", id)?;
            return Ok(());
        }
        Type::U32 => "u32",
        Type::I32 => "i32",
        Type::U64 => "u64",
        Type::I64 => "i64",
        Type::F32 => "f32",
        Type::Str => "&'a str",
        Type::OptStr => "Option<&'a str>",
        Type::Fd => "Rc<OwnedFd>",
    };
    write!(f, "{}", ty)?;
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
    obj: &str,
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
            Type::OptStr | Type::Str | Type::Fd => "{:?}",
            Type::Id(_) => "{:x}",
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

fn write_message<W: Write>(f: &mut W, obj: &str, message: &Message) -> Result<()> {
    let has_reference_type = message.has_reference_type;
    let uppercase = message.name.to_ascii_uppercase();
    writeln!(f)?;
    writeln!(f, "    pub const {}: u32 = {};", uppercase, message.id)?;
    write_message_type(f, obj, message, has_reference_type)?;
    let lifetime = if has_reference_type { "<'a>" } else { "" };
    let lifetime_b = if has_reference_type { "<'b>" } else { "" };
    let parser = if message.fields.len() > 0 {
        "parser"
    } else {
        "_parser"
    };
    writeln!(
        f,
        "    impl<'a> EiRequestParser<'a> for {}{} {{",
        message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        type Generic<'b> = {}{};",
        message.camel_name, lifetime_b,
    )?;
    writeln!(
        f,
        "        fn parse({}: &mut EiMsgParser<'_, 'a>) -> Result<Self, EiMsgParserError> {{",
        parser
    )?;
    writeln!(f, "            Ok(Self {{")?;
    writeln!(f, "                self_id: {}Id::NONE,", obj)?;
    for field in &message.fields {
        let p = match &field.val.ty.val {
            Type::Id(_) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::U64 => "ulong",
            Type::I64 => "long",
            Type::F32 => "float",
            Type::OptStr => "optstr",
            Type::Str => "str",
            Type::Fd => "fd",
        };
        writeln!(f, "                {}: parser.{}()?,", field.val.name, p)?;
    }
    writeln!(f, "            }})")?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    writeln!(
        f,
        "    impl{} EiEventFormatter for {}{} {{",
        lifetime, message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        fn format(self, fmt: &mut EiMsgFormatter<'_>) {{"
    )?;
    writeln!(f, "            fmt.header(self.self_id, {});", uppercase)?;
    fn write_fmt_expr<W: Write>(f: &mut W, prefix: &str, ty: &Type, access: &str) -> Result<()> {
        let p = match ty {
            Type::Id(_) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::U64 => "ulong",
            Type::I64 => "long",
            Type::F32 => "float",
            Type::OptStr => "optstr",
            Type::Str => "string",
            Type::Fd => "fd",
        };
        writeln!(f, "            {prefix}fmt.{p}({access});")?;
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
    writeln!(f, "        fn id(&self) -> EiObjectId {{")?;
    writeln!(f, "            self.self_id.into()")?;
    writeln!(f, "        }}")?;
    writeln!(f, "        fn interface(&self) -> EiInterface {{")?;
    writeln!(f, "            {}", obj)?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_request_handler<W: Write>(
    f: &mut W,
    camel_obj_name: &str,
    messages: &ParseResult,
) -> Result<()> {
    writeln!(f)?;
    writeln!(
        f,
        "    pub trait {camel_obj_name}RequestHandler: crate::ei::ei_object::EiObject + Sized {{"
    )?;
    writeln!(f, "        type Error: std::error::Error;")?;
    for message in &messages.requests {
        let msg = &message.val;
        let lt = match msg.has_reference_type {
            true => "<'_>",
            false => "",
        };
        writeln!(f)?;
        writeln!(
            f,
            "        fn {}(&self, req: {}{lt}, _slf: &Rc<Self>) -> Result<(), Self::Error>;",
            msg.safe_name, msg.camel_name
        )?;
    }
    writeln!(f)?;
    writeln!(f, "        #[inline(always)]")?;
    writeln!(f, "        fn handle_request_impl(")?;
    writeln!(f, "            self: Rc<Self>,")?;
    writeln!(f, "            client: &crate::ei::ei_client::EiClient,")?;
    writeln!(f, "            req: u32,")?;
    writeln!(
        f,
        "            parser: crate::utils::buffd::EiMsgParser<'_, '_>,"
    )?;
    writeln!(
        f,
        "        ) -> Result<(), crate::ei::ei_client::EiClientError> {{"
    )?;
    if messages.requests.is_empty() {
        writeln!(f, "            #![allow(unused_variables)]")?;
        writeln!(
            f,
            "            Err(crate::ei::ei_client::EiClientError::InvalidMethod)"
        )?;
    } else {
        writeln!(f, "            let method;")?;
        writeln!(
            f,
            "            let error: Box<dyn std::error::Error> = match req {{"
        )?;
        for message in &messages.requests {
            let msg = &message.val;
            write!(f, "                {} ", msg.id)?;
            let mut have_cond = false;
            if let Some(since) = msg.attribs.since {
                write!(f, "if self.version() >= {since} ")?;
                have_cond = true;
            }
            if let Some(context) = msg.attribs.context {
                if have_cond {
                    write!(f, "&&")?;
                } else {
                    write!(f, "if")?;
                }
                write!(f, " self.context() == EiContext::{context} ")?;
            }
            writeln!(f, "=> {{")?;
            writeln!(f, "                    method = \"{}\";", msg.name)?;
            writeln!(
                f,
                "                    match client.parse(&*self, parser) {{"
            )?;
            writeln!(
                f,
                "                        Ok(req) => match self.{}(req, &self) {{",
                msg.safe_name
            )?;
            writeln!(f, "                            Ok(()) => return Ok(()),")?;
            writeln!(f, "                            Err(e) => Box::new(e),")?;
            writeln!(f, "                        }},")?;
            writeln!(
                f,
                "                        Err(e) => Box::new(crate::ei::ei_client::EiParserError(e)),"
            )?;
            writeln!(f, "                    }}")?;
            writeln!(f, "                }},")?;
        }
        writeln!(
            f,
            "                _ => return Err(crate::ei::ei_client::EiClientError::InvalidMethod),"
        )?;
        writeln!(f, "            }};")?;
        writeln!(
            f,
            "            Err(crate::ei::ei_client::EiClientError::MethodError {{"
        )?;
        writeln!(f, "                interface: {camel_obj_name},")?;
        writeln!(f, "                method,")?;
        writeln!(f, "                error,")?;
        writeln!(f, "            }})")?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_file<W: Write>(f: &mut W, file: &DirEntry, obj_names: &mut Vec<String>) -> Result<()> {
    let file_name = file.file_name();
    let file_name = std::str::from_utf8(file_name.as_bytes())?;
    println!("cargo:rerun-if-changed=wire-ei/{}", file_name);
    let obj_name = file_name.split(".").next().unwrap();
    obj_names.push(obj_name.to_string());
    let camel_obj_name = to_camel(obj_name);
    writeln!(f)?;
    writeln!(f, "ei_id!({}Id);", camel_obj_name)?;
    writeln!(f)?;
    writeln!(
        f,
        "pub const {}: EiInterface = EiInterface(\"{}\");",
        camel_obj_name, obj_name
    )?;
    let contents = std::fs::read(file.path())?;
    let messages = parse_messages(&contents)?;
    writeln!(f)?;
    writeln!(f, "pub mod {} {{", obj_name)?;
    writeln!(f, "    use super::*;")?;
    for message in messages.requests.iter().chain(messages.events.iter()) {
        write_message(f, &camel_obj_name, &message.val)?;
    }
    write_request_handler(f, &camel_obj_name, &messages)?;
    writeln!(f, "}}")?;
    Ok(())
}

fn write_interface_versions<W: Write>(f: &mut W, obj_names: &[String]) -> Result<()> {
    writeln!(f)?;
    writeln!(f, "pub struct EiInterfaceVersions {{")?;
    for obj_name in obj_names {
        writeln!(f, "    pub {obj_name}: EiInterfaceVersion,")?;
    }
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "impl EiInterfaceVersions {{")?;
    writeln!(
        f,
        "    pub fn for_each(&self, mut f: impl FnMut(EiInterface, &EiInterfaceVersion)) {{"
    )?;
    for obj_name in obj_names {
        let camel = to_camel(obj_name);
        writeln!(f, "        f(crate::wire_ei::{camel}, &self.{obj_name});")?;
    }
    writeln!(f, "    }}")?;
    writeln!(f)?;
    writeln!(
        f,
        "    pub fn match_(&self, name: &str, f: impl FnOnce(&EiInterfaceVersion)) -> bool {{"
    )?;
    writeln!(f, "        match name {{")?;
    for obj_name in obj_names {
        writeln!(f, "            \"{obj_name}\" => f(&self.{obj_name}),")?;
    }
    writeln!(f, "            _ => return false,")?;
    writeln!(f, "        }}")?;
    writeln!(f, "        true")?;
    writeln!(f, "    }}")?;
    for obj_name in obj_names {
        writeln!(f)?;
        writeln!(f, "    #[allow(clippy::allow_attributes, dead_code)]")?;
        writeln!(f, "    pub fn {obj_name}(&self) -> EiVersion {{")?;
        writeln!(f, "        self.{obj_name}.version.get()")?;
        writeln!(f, "    }}")?;
    }
    writeln!(f, "}}")?;
    Ok(())
}

pub fn main() -> Result<()> {
    let mut f = open("wire_ei.rs")?;
    writeln!(f, "use std::rc::Rc;")?;
    writeln!(f, "use uapi::OwnedFd;")?;
    writeln!(f, "use crate::ei::{{EiContext, EiInterfaceVersion}};")?;
    writeln!(
        f,
        "use crate::ei::ei_client::{{EiEventFormatter, EiRequestParser}};"
    )?;
    writeln!(
        f,
        "use crate::ei::ei_object::{{EiObjectId, EiInterface, EiVersion}};"
    )?;
    writeln!(
        f,
        "use crate::utils::buffd::{{EiMsgFormatter, EiMsgParser, EiMsgParserError}};"
    )?;
    println!("cargo:rerun-if-changed=wire-ei");
    let mut files = vec![];
    for file in std::fs::read_dir("wire-ei")? {
        files.push(file?);
    }
    files.sort_by_key(|f| f.file_name());
    let mut obj_names = vec![];
    for file in files {
        write_file(&mut f, &file, &mut obj_names)
            .with_context(|| format!("While processing {}", file.path().display()))?;
    }
    write_interface_versions(&mut f, &obj_names).context("Could not write interface versions")?;
    Ok(())
}

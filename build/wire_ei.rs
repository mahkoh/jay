use crate::indent::Indent;
use crate::open;
use crate::tokens::Symbol;
use crate::tokens::Token;
use crate::tokens::TokenKind;
use crate::tokens::TreeDelim;
use crate::tokens::tokenize;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use std::fs::DirEntry;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

#[derive(Debug)]
struct Lined<T> {
    #[expect(unused)]
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
    dead: bool,
    requests: Vec<Lined<Message>>,
    events: Vec<Lined<Message>>,
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<ParseResult> {
        let mut dead = false;
        let mut requests = vec![];
        let mut events = vec![];
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            let res = match ty.as_bytes() {
                b"dead" => {
                    dead = true;
                    continue;
                }
                b"request" => &mut requests,
                b"event" => &mut events,
                _ => bail!("In line {}: Unexpected entry {:?}", line, ty),
            };
            res.push(self.parse_message(res.len() as _)?);
        }
        Ok(ParseResult {
            dead,
            requests,
            events,
        })
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
    define_w!(f, w, wl);
    let ty = match ty {
        Type::Id(id) => {
            w!("{}Id", id);
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
    w!("{}", ty);
    Ok(())
}

fn write_field<W: Write>(f: &mut W, xn: &Indent, field: &Field) -> Result<()> {
    define_w!(f, w, wl);
    w!("{xn}pub {}: ", field.name);
    write_type(f, &field.ty.val)?;
    wl!(",");
    Ok(())
}

fn write_message_type<W: Write>(
    f: &mut W,
    xn: &Indent,
    obj: &str,
    message: &Message,
    needs_lifetime: bool,
) -> Result<()> {
    define_w!(f, w, wl);
    let lifetime = if needs_lifetime { "<'a>" } else { "" };
    wl!("{xn}pub struct {}{} {{", message.camel_name, lifetime);
    {
        push_xn!(xn);
        wl!("{xn}pub self_id: {}Id,", obj);
        for field in &message.fields {
            write_field(f, xn, &field.val)?;
        }
    }
    wl!("{xn}}}");
    wl!(
        "{xn}impl{} std::fmt::Debug for {}{} {{",
        lifetime,
        message.camel_name,
        lifetime
    );
    {
        push_xn!(xn);
        wl!("{xn}fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{");
        {
            push_xn!(xn);
            w!(r#"{xn}write!(fmt, "{}("#, message.name);
            for (i, field) in message.fields.iter().enumerate() {
                if i > 0 {
                    w!(", ");
                }
                let formatter = match &field.val.ty.val {
                    Type::OptStr | Type::Str | Type::Fd => "{:?}",
                    Type::Id(_) => "{:x}",
                    _ => "{}",
                };
                w!("{}: {}", field.val.name, formatter);
            }
            w!(r#")""#);
            for field in &message.fields {
                w!(", self.{}", field.val.name);
            }
            wl!(r")");
        }
        wl!("{xn}}}");
    }
    wl!("{xn}}}");
    Ok(())
}

fn write_message<W: Write>(f: &mut W, xn: &Indent, obj: &str, message: &Message) -> Result<()> {
    define_w!(f, w, wl);
    let has_reference_type = message.has_reference_type;
    let uppercase = message.name.to_ascii_uppercase();
    wl!();
    wl!("{xn}pub const {}: u32 = {};", uppercase, message.id);
    write_message_type(f, xn, obj, message, has_reference_type)?;
    let lifetime = if has_reference_type { "<'a>" } else { "" };
    let lifetime_b = if has_reference_type { "<'b>" } else { "" };
    let parser = if message.fields.len() > 0 {
        "parser"
    } else {
        "_parser"
    };
    wl!(
        "{xn}impl<'a> EiRequestParser<'a> for {}{} {{",
        message.camel_name,
        lifetime
    );
    {
        push_xn!(xn);
        wl!(
            "{xn}type Generic<'b> = {}{};",
            message.camel_name,
            lifetime_b,
        );
        wl!(
            "{xn}fn parse({}: &mut EiMsgParser<'_, 'a>) -> Result<Self, EiMsgParserError> {{",
            parser
        );
        {
            push_xn!(xn);
            wl!("{xn}Ok(Self {{");
            {
                push_xn!(xn);
                wl!("{xn}self_id: {}Id::NONE,", obj);
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
                    wl!("{xn}{}: parser.{}()?,", field.val.name, p);
                }
            }
            wl!("{xn}}})");
        }
        wl!("{xn}}}");
    }
    wl!("{xn}}}");
    wl!(
        "{xn}impl{} EiEventFormatter for {}{} {{",
        lifetime,
        message.camel_name,
        lifetime
    );
    {
        push_xn!(xn);
        wl!("{xn}fn format(self, fmt: &mut EiMsgFormatter<'_>) {{");
        {
            push_xn!(xn);
            wl!("{xn}fmt.header(self.self_id, {});", uppercase);
            fn write_fmt_expr<W: Write>(
                f: &mut W,
                xn: &Indent,
                prefix: &str,
                ty: &Type,
                access: &str,
            ) -> Result<()> {
                define_w!(f, w2, wl2);
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
                wl2!("{xn}{prefix}fmt.{p}({access});");
                Ok(())
            }
            for field in &message.fields {
                write_fmt_expr(
                    f,
                    xn,
                    "",
                    &field.val.ty.val,
                    &format!("self.{}", field.val.name),
                )?;
            }
        }
        wl!("{xn}}}");
        wl!("{xn}fn id(&self) -> EiObjectId {{");
        {
            push_xn!(xn);
            wl!("{xn}self.self_id.into()");
        }
        wl!("{xn}}}");
        wl!("{xn}fn interface(&self) -> EiInterface {{");
        {
            push_xn!(xn);
            wl!("{xn}{}", obj);
        }
        wl!("{xn}}}");
    }
    wl!("{xn}}}");
    Ok(())
}

fn write_request_handler<W: Write>(
    f: &mut W,
    xn: &Indent,
    camel_obj_name: &str,
    messages: &ParseResult,
) -> Result<()> {
    define_w!(f, w, wl);
    wl!();
    if messages.dead {
        wl!("{xn}#[allow(dead_code)]");
    }
    wl!("{xn}pub trait {camel_obj_name}RequestHandler: crate::ei::ei_object::EiObject + Sized {{");
    {
        push_xn!(xn);
        wl!("{xn}type Error: std::error::Error;");
        for message in &messages.requests {
            let msg = &message.val;
            let lt = match msg.has_reference_type {
                true => "<'_>",
                false => "",
            };
            wl!();
            wl!(
                "{xn}fn {}(&self, req: {}{lt}, _slf: &Rc<Self>) -> Result<(), Self::Error>;",
                msg.safe_name,
                msg.camel_name
            );
        }
        wl!();
        wl!("{xn}#[inline(always)]");
        wl!("{xn}fn handle_request_impl(");
        {
            push_xn!(xn);
            wl!("{xn}self: Rc<Self>,");
            wl!("{xn}client: &crate::ei::ei_client::EiClient,");
            wl!("{xn}req: u32,");
            wl!("{xn}parser: crate::utils::buffd::EiMsgParser<'_, '_>,");
        }
        wl!("{xn}) -> Result<(), crate::ei::ei_client::EiClientError> {{");
        {
            push_xn!(xn);
            if messages.requests.is_empty() {
                wl!("{xn}#![allow(unused_variables)]");
                wl!("{xn}Err(crate::ei::ei_client::EiClientError::InvalidMethod)");
            } else {
                wl!("{xn}let method;");
                wl!("{xn}let error: Box<dyn std::error::Error> = match req {{");
                {
                    push_xn!(xn);
                    for message in &messages.requests {
                        let msg = &message.val;
                        w!("{xn}{} ", msg.id);
                        let mut have_cond = false;
                        if let Some(since) = msg.attribs.since {
                            w!("if self.version() >= {since} ");
                            have_cond = true;
                        }
                        if let Some(context) = msg.attribs.context {
                            if have_cond {
                                w!("&&");
                            } else {
                                w!("if");
                            }
                            w!(" self.context() == EiContext::{context} ");
                        }
                        wl!("=> {{");
                        {
                            push_xn!(xn);
                            wl!("{xn}method = \"{}\";", msg.name);
                            wl!("{xn}match client.parse(&*self, parser) {{");
                            {
                                push_xn!(xn);
                                wl!("{xn}Ok(req) => match self.{}(req, &self) {{", msg.safe_name);
                                {
                                    push_xn!(xn);
                                    wl!("{xn}Ok(()) => return Ok(()),");
                                    wl!("{xn}Err(e) => Box::new(e),");
                                }
                                wl!("{xn}}},");
                                wl!(
                                    "{xn}Err(e) => Box::new(crate::ei::ei_client::EiParserError(e)),"
                                );
                            }
                            wl!("{xn}}}");
                        }
                        wl!("{xn}}},");
                    }
                    wl!("{xn}_ => return Err(crate::ei::ei_client::EiClientError::InvalidMethod),");
                }
                wl!("{xn}}};");
                wl!("{xn}Err(crate::ei::ei_client::EiClientError::MethodError {{");
                {
                    push_xn!(xn);
                    wl!("{xn}interface: {camel_obj_name},");
                    wl!("{xn}method,");
                    wl!("{xn}error,");
                }
                wl!("{xn}}})");
            }
        }
        wl!("{xn}}}");
    }
    wl!("{xn}}}");
    Ok(())
}

fn write_file<W: Write>(f: &mut W, file: &DirEntry, obj_names: &mut Vec<String>) -> Result<()> {
    define_w!(f, w, wl);
    define_xn!(xn);
    let file_name = file.file_name();
    let file_name = std::str::from_utf8(file_name.as_bytes())?;
    println!("cargo:rerun-if-changed=wire-ei/{}", file_name);
    let obj_name = file_name.split(".").next().unwrap();
    obj_names.push(obj_name.to_string());
    let camel_obj_name = to_camel(obj_name);
    wl!();
    wl!("ei_id!({}Id);", camel_obj_name);
    wl!();
    wl!(
        "pub const {}: EiInterface = EiInterface(\"{}\");",
        camel_obj_name,
        obj_name
    );
    let contents = std::fs::read(file.path())?;
    let messages = parse_messages(&contents)?;
    wl!();
    wl!("pub mod {} {{", obj_name);
    {
        push_xn!(xn);
        wl!("{xn}use super::*;");
        for message in messages.requests.iter().chain(messages.events.iter()) {
            write_message(f, xn, &camel_obj_name, &message.val)?;
        }
        write_request_handler(f, xn, &camel_obj_name, &messages)?;
    }
    wl!("}}");
    Ok(())
}

fn write_interface_versions<W: Write>(f: &mut W, obj_names: &[String]) -> Result<()> {
    define_w!(f, w, wl);
    define_xn!(xn);
    wl!();
    wl!("pub struct EiInterfaceVersions {{");
    {
        push_xn!(xn);
        for obj_name in obj_names {
            wl!("{xn}pub {obj_name}: EiInterfaceVersion,");
        }
    }
    wl!("}}");
    wl!();
    wl!("impl EiInterfaceVersions {{");
    {
        push_xn!(xn);
        wl!("{xn}pub fn for_each(&self, mut f: impl FnMut(EiInterface, &EiInterfaceVersion)) {{");
        {
            push_xn!(xn);
            for obj_name in obj_names {
                let camel = to_camel(obj_name);
                wl!("{xn}f(crate::wire_ei::{camel}, &self.{obj_name});");
            }
        }
        wl!("{xn}}}");
        wl!();
        wl!("{xn}pub fn match_(&self, name: &str, f: impl FnOnce(&EiInterfaceVersion)) -> bool {{");
        {
            push_xn!(xn);
            wl!("{xn}match name {{");
            {
                push_xn!(xn);
                for obj_name in obj_names {
                    wl!("{xn}\"{obj_name}\" => f(&self.{obj_name}),");
                }
                wl!("{xn}_ => return false,");
            }
            wl!("{xn}}}");
            wl!("{xn}true");
        }
        wl!("{xn}}}");
        for obj_name in obj_names {
            wl!();
            wl!("{xn}#[allow(dead_code)]");
            wl!("{xn}pub fn {obj_name}(&self) -> EiVersion {{");
            {
                push_xn!(xn);
                wl!("{xn}self.{obj_name}.version.get()");
            }
            wl!("{xn}}}");
        }
    }
    wl!("}}");
    Ok(())
}

pub fn main() -> Result<()> {
    let mut f = open("wire_ei.rs")?;
    define_w!(f, w, wl);
    wl!("use std::rc::Rc;");
    wl!("use uapi::OwnedFd;");
    wl!("use crate::ei::{{EiContext, EiInterfaceVersion}};");
    wl!("use crate::ei::ei_client::{{EiEventFormatter, EiRequestParser}};");
    wl!("use crate::ei::ei_object::{{EiObjectId, EiInterface, EiVersion}};");
    wl!("use crate::utils::buffd::{{EiMsgFormatter, EiMsgParser, EiMsgParserError}};");
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

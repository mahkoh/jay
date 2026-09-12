mod client_trace;
mod parser;
mod singletons;
mod synthetic;

use crate::indent::Indent;
use crate::open;
use crate::str_table::Interned;
use crate::str_table::intern;
use crate::wire::client_trace::write_client_trace_files;
use crate::wire::parser::Field;
use crate::wire::parser::Lined;
use crate::wire::parser::Message;
use crate::wire::parser::ParseResult;
use crate::wire::parser::Type;
use crate::wire::parser::parse_messages;
use crate::wire::parser::to_camel;
use crate::wire::singletons::write_singletons;
use crate::wire::synthetic::write_synthetic_helpers;
use anyhow::Context;
use anyhow::Result;
use std::env;
use std::fmt;
use std::fs::DirEntry;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    define_w!(f, w, wl);
    match ty {
        Type::Id(_, id) => w!("{}Id", id),
        Type::U32 => w!("u32"),
        Type::I32 => w!("i32"),
        Type::U64 => w!("u64"),
        Type::U64Rev => w!("u64"),
        Type::Str => w!("&'a str"),
        Type::OptStr => w!("Option<&'a str>"),
        Type::BStr => w!("&'a BStr"),
        Type::Fixed => w!("Fixed"),
        Type::Fd => w!("Rc<OwnedFd>"),
        Type::Bool => w!("bool"),
        Type::Array(n) => {
            w!("&'a [");
            write_type(f, n)?;
            w!("]");
        }
        Type::Pod(p) => f.write_all(p.as_bytes())?,
    }
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
    obj: &str,
    message: &Message,
    needs_lifetime: bool,
) -> Result<()> {
    define_w!(f, w, wl);
    define_xn!(xn);
    let lifetime = if needs_lifetime { "<'a>" } else { "" };
    if message.attribs.dead {
        wl!(r#"{xn}#[cfg_attr(feature = "it", allow(unused))]"#);
        wl!(r#"{xn}#[cfg_attr(not(feature = "it"), expect(unused))]"#);
    }
    wl!("pub struct {}{} {{", message.camel_name, lifetime);
    {
        push_xn!(xn);
        wl!("{xn}pub self_id: {}Id,", obj);
        for field in &message.fields {
            write_field(f, xn, &field.val)?;
        }
    }
    wl!("}}");
    Ok(())
}

fn write_message<W: Write>(f: &mut W, obj: &str, message: &Message) -> Result<()> {
    define_w!(f, w, wl);
    define_xn!(xn);
    let has_reference_type = message.has_reference_type;
    let uppercase = message.name.raw().to_ascii_uppercase();
    wl!();
    if message.attribs.dead {
        wl!(r#"{xn}#[cfg_attr(feature = "it", allow(unused))]"#);
        wl!(r#"{xn}#[cfg_attr(not(feature = "it"), expect(unused))]"#);
    }
    wl!("pub const {}: u32 = {};", uppercase, message.id);
    write_message_type(f, obj, message, has_reference_type)?;
    let lifetime = if has_reference_type { "<'a>" } else { "" };
    let lifetime_b = if has_reference_type { "<'b>" } else { "" };
    wl!(
        "impl<'a> RequestParser<'a> for {}{} {{",
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
        wl!("{xn}const ID: u32 = {};", message.id);
        wl!("{xn}fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {{");
        {
            push_xn!(xn);
            if message.is_fixed_size {
                if message.num_ids > 0 {
                    let xn2 = &xn.push();
                    wl!("{xn}if parser.wide {{");
                    write_fixed_parse_body(f, xn2, message, obj, true)?;
                    wl!("{xn}}} else {{");
                    write_fixed_parse_body(f, xn2, message, obj, false)?;
                    wl!("{xn}}}");
                } else {
                    write_fixed_parse_body(f, xn, message, obj, false)?;
                }
            } else {
                write_variable_parse_body(f, xn, message, obj)?;
            }
        }
        wl!("{xn}}}");
    }
    wl!("}}");
    wl!(
        "impl{} EventFormatter for {}{} {{",
        lifetime,
        message.camel_name,
        lifetime
    );
    {
        push_xn!(xn);
        wl!("{xn}const NUM_FDS: u32 = {};", message.num_fds);
        wl!("{xn}#[inline]");
        wl!("{xn}fn format(self, fmt: &mut MsgFormatter<'_>) {{");
        {
            push_xn!(xn);
            if message.is_fixed_size {
                if message.num_ids > 0 {
                    let xn2 = &xn.push();
                    wl!("{xn}if fmt.wide {{");
                    write_fixed_format_body(f, xn2, message, &uppercase, true)?;
                    wl!("{xn}}} else {{");
                    write_fixed_format_body(f, xn2, message, &uppercase, false)?;
                    wl!("{xn}}}");
                } else {
                    write_fixed_format_body(f, xn, message, &uppercase, false)?;
                }
            } else {
                write_variable_format_body(f, xn, message, &uppercase)?;
            }
        }
        wl!("{xn}}}");
        wl!("{xn}fn id(&self) -> ObjectId {{");
        {
            push_xn!(xn);
            wl!("{xn}ObjectId(self.self_id.0)");
        }
        wl!("{xn}}}");
    }
    wl!("}}");
    Ok(())
}

fn write_fixed_parse_body<W: Write>(
    f: &mut W,
    xn: &Indent,
    message: &Message,
    obj: &str,
    wide: bool,
) -> Result<()> {
    define_w!(f, w, wl);
    wl!("{xn}let [");
    {
        push_xn!(xn);
        for (i, field) in message.fields.iter().enumerate() {
            match &field.val.ty.val {
                Type::Id(..) if wide => {
                    wl!("{xn}arg{i}_lo,");
                    wl!("{xn}arg{i}_hi,");
                }
                Type::U64 => {
                    wl!("{xn}arg{i}_hi,");
                    wl!("{xn}arg{i}_lo,");
                }
                Type::U64Rev => {
                    wl!("{xn}arg{i}_lo,");
                    wl!("{xn}arg{i}_hi,");
                }
                Type::Fd => {}
                _ => {
                    wl!("{xn}arg{i},");
                }
            }
        }
    }
    wl!("{xn}] = *parser.data() else {{");
    {
        push_xn!(xn);
        wl!("{xn}return Err(MsgParserError::UnexpectedMessageSize);");
    }
    wl!("{xn}}};");
    wl!("{xn}Ok(Self {{");
    {
        push_xn!(xn);
        wl!("{xn}self_id: {}Id::NONE,", obj);
        for (i, field) in message.fields.iter().enumerate() {
            wl!(
                "{xn}{}: {},",
                field.val.name,
                fmt::from_fn(|f| {
                    define_w!(f, w2, wl2);
                    match &field.val.ty.val {
                        Type::Id(_, name) if wide => {
                            w2!("{name}Id(((arg{i}_hi as u64) << 32) | (arg{i}_lo as u64))")
                        }
                        Type::Id(_, name) => w2!("{name}Id(arg{i} as u64)"),
                        Type::U32 => w2!("arg{i}"),
                        Type::I32 => w2!("arg{i} as i32"),
                        Type::U64 | Type::U64Rev => {
                            w2!("((arg{i}_hi as u64) << 32) | (arg{i}_lo as u64)")
                        }
                        Type::OptStr => unreachable!(),
                        Type::Str => unreachable!(),
                        Type::Fixed => w2!("Fixed(arg{i} as i32)"),
                        Type::Fd => w2!("parser.fd()?"),
                        Type::Bool => w2!("arg{i} != 0"),
                        Type::BStr => unreachable!(),
                        Type::Array(_) => unreachable!(),
                        Type::Pod(_) => unreachable!(),
                    }
                    Ok(())
                })
            );
        }
    }
    wl!("{xn}}})");
    Ok(())
}

fn write_variable_parse_body<W: Write>(
    f: &mut W,
    xn: &Indent,
    message: &Message,
    obj: &str,
) -> Result<()> {
    define_w!(f, w, wl);
    wl!("{xn}let res = Ok(Self {{");
    {
        push_xn!(xn);
        wl!("{xn}self_id: {}Id::NONE,", obj);
        for field in &message.fields {
            let p = match &field.val.ty.val {
                Type::Id(..) => "object",
                Type::U32 => "uint",
                Type::I32 => "int",
                Type::U64 => "u64",
                Type::U64Rev => "u64_rev",
                Type::OptStr => "optstr",
                Type::Str => "str",
                Type::Fixed => "fixed",
                Type::Fd => "fd",
                Type::Bool => "bool",
                Type::BStr => "bstr",
                Type::Array(_) => "binary_array",
                Type::Pod(_) => "binary",
            };
            wl!("{xn}{}: parser.{}()?,", field.val.name, p);
        }
    }
    wl!("{xn}}});");
    wl!("{xn}parser.eof()?;");
    wl!("{xn}res");
    Ok(())
}

fn write_fixed_format_body<W: Write>(
    f: &mut W,
    xn: &Indent,
    message: &Message,
    uppercase: &str,
    wide: bool,
) -> Result<()> {
    define_w!(f, w, wl);
    wl!("{xn}fmt.data(&[");
    {
        push_xn!(xn);
        wl!("{xn}self.self_id.0 as u32,");
        wl!("{xn}{uppercase},");
        for field in &message.fields {
            let prefix = format!("{xn}self.{}", field.val.name);
            match &field.val.ty.val {
                Type::Id(_, _) => {
                    wl!("{prefix}.0 as u32,");
                    if wide {
                        wl!("{xn}(self.{}.0 >> 32) as u32,", field.val.name);
                    }
                }
                Type::U32 => wl!("{prefix},"),
                Type::I32 => wl!("{prefix} as u32,"),
                Type::U64 => {
                    wl!("{xn}(self.{} >> 32) as u32,", field.val.name);
                    wl!("{prefix} as u32,");
                }
                Type::U64Rev => {
                    wl!("{prefix} as u32,");
                    wl!("{xn}(self.{} >> 32) as u32,", field.val.name);
                }
                Type::Str => unreachable!(),
                Type::OptStr => unreachable!(),
                Type::BStr => unreachable!(),
                Type::Fixed => wl!("{prefix}.0 as u32,"),
                Type::Fd => {}
                Type::Bool => wl!("{prefix} as u32,"),
                Type::Array(_) => unreachable!(),
                Type::Pod(_) => unreachable!(),
            }
        }
    }
    wl!("{xn}]);");
    for field in &message.fields {
        if let Type::Fd = &field.val.ty.val {
            wl!("{xn}fmt.fd(self.{});", field.val.name);
        }
    }
    Ok(())
}

fn write_variable_format_body<W: Write>(
    f: &mut W,
    xn: &Indent,
    message: &Message,
    uppercase: &str,
) -> Result<()> {
    define_w!(f, w, wl);
    wl!("{xn}fmt.header(self.self_id, {});", uppercase);
    fn write_fmt_expr<W: Write>(f: &mut W, xn: &Indent, ty: &Type, access: &str) -> Result<()> {
        define_w!(f, w2, wl2);
        let p = match ty {
            Type::Id(..) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::U64 => "u64",
            Type::U64Rev => "u64_rev",
            Type::OptStr => "optstr",
            Type::Str | Type::BStr => "string",
            Type::Fixed => "fixed",
            Type::Fd => "fd",
            Type::Bool => "bool",
            Type::Array(..) => "binary",
            Type::Pod(..) => "binary",
        };
        let rf = match ty {
            Type::Pod(..) => "&",
            _ => "",
        };
        wl2!("{xn}fmt.{}({}{});", p, rf, access);
        Ok(())
    }
    for field in &message.fields {
        write_fmt_expr(
            f,
            xn,
            &field.val.ty.val,
            &format!("self.{}", field.val.name),
        )?;
    }
    Ok(())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RequestHandlerDirection {
    Request,
    Event,
}

fn write_request_handler<W: Write>(
    f: &mut W,
    camel_obj_name: &str,
    messages: &[Lined<Message>],
    direction: RequestHandlerDirection,
    dead: bool,
    it_only: bool,
) -> Result<()> {
    define_w!(f, w, wl);
    define_xn!(xn);
    let snake_direction;
    let camel_direction;
    let parent;
    let error;
    let param;
    let version;
    wl!();
    match direction {
        RequestHandlerDirection::Request => {
            snake_direction = "request";
            camel_direction = "Request";
            parent = "crate::object::Object";
            error = "crate::client::ClientError";
            param = "req";
            version = "self.version()";
            if dead {
                wl!("#[allow(dead_code)]");
            }
        }
        RequestHandlerDirection::Event => {
            snake_direction = "event";
            camel_direction = "Event";
            parent = "'static";
            error = "crate::object::EventHandlingError";
            param = "ev";
            version = "version";
        }
    }
    if it_only {
        wl!(r#"#[cfg(feature = "it")]"#);
        wl!(r#"#[allow(dead_code)]"#);
    }
    wl!("pub trait {camel_obj_name}{camel_direction}Handler: Sized + {parent} {{");
    {
        push_xn!(xn);
        wl!("{xn}type Error: std::error::Error;");
        for message in messages {
            let msg = &message.val;
            let lt = match msg.has_reference_type {
                true => "<'_>",
                false => "",
            };
            wl!();
            wl!(
                "{xn}fn {}(&self, {param}: {}{lt}, _slf: &Rc<Self>) -> Result<(), Self::Error>;",
                msg.safe_name,
                msg.camel_name
            );
        }
        wl!();
        if direction == RequestHandlerDirection::Request {
            wl!("{xn}#[inline(always)]");
        }
        wl!("{xn}fn handle_{snake_direction}_impl(");
        {
            push_xn!(xn);
            wl!("{xn}self: Rc<Self>,");
            match direction {
                RequestHandlerDirection::Request => {
                    wl!("{xn}client: &crate::client::Client,");
                }
                RequestHandlerDirection::Event => {
                    wl!("{xn}id: ObjectId,");
                    wl!("{xn}#[allow(unused_variables)]");
                    wl!("{xn}version: crate::object::Version,");
                }
            }
            wl!("{xn}req: u32,");
            match direction {
                RequestHandlerDirection::Request => {
                    wl!("{xn}parser: crate::utils::buffd::MsgParser<'_, '_>,");
                }
                RequestHandlerDirection::Event => {
                    wl!("{xn}#[allow(unused_mut)]");
                    wl!("{xn}mut parser: crate::utils::buffd::MsgParser<'_, '_>,");
                }
            }
        }
        wl!("{xn}) -> Result<(), {error}> {{");
        {
            push_xn!(xn);
            if messages.is_empty() {
                wl!("{xn}#![allow(unused_variables)]");
                wl!("{xn}Err({error}::InvalidMethod)");
            } else {
                wl!("{xn}let method;");
                wl!("{xn}let error: Box<dyn std::error::Error> = match req {{");
                {
                    push_xn!(xn);
                    for message in messages {
                        let msg = &message.val;
                        w!("{xn}{} ", msg.id);
                        if let Some(since) = msg.attribs.since {
                            w!("if {version} >= {since} ");
                        }
                        wl!("=> {{");
                        {
                            push_xn!(xn);
                            wl!("{xn}method = {};", msg.name);
                            match direction {
                                RequestHandlerDirection::Request => {
                                    wl!("{xn}match client.parse(&*self, parser) {{");
                                }
                                RequestHandlerDirection::Event => {
                                    wl!("{xn}match {}::parse(&mut parser) {{", msg.camel_name);
                                }
                            }
                            {
                                push_xn!(xn);
                                wl!(
                                    "{xn}Ok(req) => match {camel_obj_name}{camel_direction}Handler::{}(&*self, req, &self) {{",
                                    msg.safe_name,
                                );
                                {
                                    push_xn!(xn);
                                    wl!("{xn}Ok(()) => return Ok(()),");
                                    wl!("{xn}Err(e) => Box::new(e),");
                                }
                                wl!("{xn}}},");
                                wl!("{xn}Err(e) => Box::new(crate::client::ParserError(e)),");
                            }
                            wl!("{xn}}}");
                        }
                        wl!("{xn}}},");
                    }
                    wl!("{xn}_ => return Err({error}::InvalidMethod),");
                }
                wl!("{xn}}};");
                wl!("{xn}Err({error}::MethodError {{");
                {
                    push_xn!(xn);
                    wl!("{xn}interface: {camel_obj_name},");
                    match direction {
                        RequestHandlerDirection::Request => {
                            wl!("{xn}id: self.id(),");
                        }
                        RequestHandlerDirection::Event => {
                            wl!("{xn}id,");
                        }
                    }
                    wl!("{xn}method,");
                    wl!("{xn}error,");
                }
                wl!("{xn}}})");
            }
        }
        wl!("{xn}}}");
    }
    wl!("}}");
    Ok(())
}

struct ParsedFile {
    obj_name: Interned,
    camel_obj_name: String,
    messages: ParseResult,
}

fn parse_file(file: &DirEntry) -> Result<ParsedFile> {
    let file_name = file.file_name();
    let file_name = std::str::from_utf8(file_name.as_bytes())?;
    println!("cargo:rerun-if-changed=wire/{}", file_name);
    let obj_name = file_name.split(".").next().unwrap();
    let camel_obj_name = to_camel(obj_name);
    let contents = std::fs::read(file.path())?;
    let messages = parse_messages(&contents)?;
    Ok(ParsedFile {
        obj_name: intern(obj_name),
        camel_obj_name,
        messages,
    })
}

fn write_file(f: &mut impl Write, file: &ParsedFile) -> Result<()> {
    define_w!(f, w, wl);
    let ParsedFile {
        obj_name,
        camel_obj_name,
        messages,
    } = file;
    wl!();
    wl!("id!({}Id);", camel_obj_name);
    wl!();
    wl!(
        "pub const {}: Interface = Interface({});",
        camel_obj_name,
        obj_name
    );
    wl!();
    wl!("pub mod {};", obj_name.raw());
    {
        let f = &mut open(&format!("wire/{}.rs", obj_name.raw()))?;
        define_w!(f, w2, wl2);
        wl2!("use super::*;");
        for message in messages.messages() {
            write_message(f, camel_obj_name, &message.val)?;
        }
        write_request_handler(
            f,
            camel_obj_name,
            &messages.requests,
            RequestHandlerDirection::Request,
            messages.dead,
            false,
        )?;
        write_request_handler(
            f,
            camel_obj_name,
            &messages.events,
            RequestHandlerDirection::Event,
            messages.dead,
            !messages.event_handler,
        )?;
    }
    Ok(())
}

pub fn main() -> Result<()> {
    std::fs::create_dir_all(Path::new(&env::var("OUT_DIR").unwrap()).join("wire"))?;
    let mut f = open("wire/mod.rs")?;
    define_w!(f, w, wl);
    wl!("use std::rc::Rc;");
    wl!("use uapi::OwnedFd;");
    wl!("use bstr::BStr;");
    wl!("use crate::fixed::Fixed;");
    wl!("use crate::client::{{EventFormatter, RequestParser}};");
    wl!("use crate::object::Interface;");
    wl!("use crate::utils::buffd::{{MsgFormatter, MsgParser, MsgParserError}};");
    wl!("use crate::utils::str_table::StrAccess;");
    println!("cargo:rerun-if-changed=wire");
    let mut files = vec![];
    for file in std::fs::read_dir("wire")? {
        files.push(file?);
    }
    files.sort_by_key(|f| f.file_name());
    let mut parsed_files = vec![];
    for file in files {
        let parsed = parse_file(&file)
            .with_context(|| format!("While processing {}", file.path().display()))?;
        parsed_files.push(parsed);
    }
    write_client_trace_files(&parsed_files)?;
    write_singletons(&parsed_files)?;
    write_synthetic_helpers(&parsed_files)?;
    for file in &parsed_files {
        write_file(&mut f, file)?;
    }
    let max_message_ids = parsed_files
        .iter()
        .flat_map(|f| f.messages.requests.iter().chain(f.messages.events.iter()))
        .map(|m| m.val.num_ids)
        .max()
        .unwrap_or(0);
    wl!();
    wl!("pub const MAX_MESSAGE_IDS: usize = {max_message_ids};");
    Ok(())
}

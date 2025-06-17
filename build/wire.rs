mod parser;

use {
    crate::{
        open,
        wire::parser::{Field, Lined, Message, Type, parse_messages, to_camel},
    },
    anyhow::{Context, Result},
    std::{fs::DirEntry, io::Write, os::unix::ffi::OsStrExt},
};

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    match ty {
        Type::Id(_, id) => write!(f, "{}Id", id)?,
        Type::U32 => write!(f, "u32")?,
        Type::I32 => write!(f, "i32")?,
        Type::U64 => write!(f, "u64")?,
        Type::U64Rev => write!(f, "u64")?,
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
        "    impl<'a> RequestParser<'a> for {}{} {{",
        message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        type Generic<'b> = {}{};",
        message.camel_name, lifetime_b,
    )?;
    writeln!(f, "        const ID: u32 = {};", message.id)?;
    writeln!(
        f,
        "        fn parse({}: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {{",
        parser
    )?;
    writeln!(f, "            Ok(Self {{")?;
    writeln!(f, "                self_id: {}Id::NONE,", obj)?;
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
            Type::Id(..) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::U64 => "u64",
            Type::U64Rev => "u64_rev",
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
) -> Result<()> {
    let snake_direction;
    let camel_direction;
    let parent;
    let parser;
    let error;
    let param;
    writeln!(f)?;
    match direction {
        RequestHandlerDirection::Request => {
            snake_direction = "request";
            camel_direction = "Request";
            parent = "crate::object::Object";
            parser = "crate::client::Client";
            error = "crate::client::ClientError";
            param = "req";
        }
        RequestHandlerDirection::Event => {
            snake_direction = "event";
            camel_direction = "Event";
            parent = "crate::wl_usr::usr_object::UsrObject";
            parser = "crate::wl_usr::UsrCon";
            error = "crate::wl_usr::UsrConError";
            param = "ev";
            writeln!(f, "    #[allow(clippy::allow_attributes, dead_code)]")?;
        }
    }
    writeln!(
        f,
        "    pub trait {camel_obj_name}{camel_direction}Handler: {parent} + Sized {{"
    )?;
    writeln!(f, "        type Error: std::error::Error;")?;
    for message in messages {
        let msg = &message.val;
        let lt = match msg.has_reference_type {
            true => "<'_>",
            false => "",
        };
        writeln!(f)?;
        writeln!(
            f,
            "        fn {}(&self, {param}: {}{lt}, _slf: &Rc<Self>) -> Result<(), Self::Error>;",
            msg.safe_name, msg.camel_name
        )?;
    }
    writeln!(f)?;
    writeln!(f, "        #[inline(always)]")?;
    writeln!(f, "        fn handle_{snake_direction}_impl(")?;
    writeln!(f, "            self: Rc<Self>,")?;
    writeln!(f, "            client: &{parser},")?;
    writeln!(f, "            req: u32,")?;
    writeln!(
        f,
        "            parser: crate::utils::buffd::MsgParser<'_, '_>,"
    )?;
    writeln!(f, "        ) -> Result<(), {error}> {{")?;
    if messages.is_empty() {
        writeln!(f, "            #![allow(unused_variables)]")?;
        writeln!(f, "            Err({error}::InvalidMethod)")?;
    } else {
        writeln!(f, "            let method;")?;
        writeln!(
            f,
            "            let error: Box<dyn std::error::Error> = match req {{"
        )?;
        for message in messages {
            let msg = &message.val;
            write!(f, "                {} ", msg.id)?;
            if let Some(since) = msg.attribs.since {
                write!(f, "if self.version() >= {since} ")?;
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
                "                        Err(e) => Box::new(crate::client::ParserError(e)),"
            )?;
            writeln!(f, "                    }}")?;
            writeln!(f, "                }},")?;
        }
        writeln!(
            f,
            "                _ => return Err({error}::InvalidMethod),"
        )?;
        writeln!(f, "            }};")?;
        writeln!(f, "            Err({error}::MethodError {{")?;
        writeln!(f, "                interface: {camel_obj_name},")?;
        writeln!(f, "                id: self.id(),")?;
        writeln!(f, "                method,")?;
        writeln!(f, "                error,")?;
        writeln!(f, "            }})")?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_file<W: Write>(f: &mut W, file: &DirEntry) -> Result<()> {
    let file_name = file.file_name();
    let file_name = std::str::from_utf8(file_name.as_bytes())?;
    println!("cargo:rerun-if-changed=wire/{}", file_name);
    let obj_name = file_name.split(".").next().unwrap();
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
    writeln!(f)?;
    writeln!(f, "pub mod {} {{", obj_name)?;
    writeln!(f, "    use super::*;")?;
    for message in messages.requests.iter().chain(messages.events.iter()) {
        write_message(f, &camel_obj_name, &message.val)?;
    }
    write_request_handler(
        f,
        &camel_obj_name,
        &messages.requests,
        RequestHandlerDirection::Request,
    )?;
    write_request_handler(
        f,
        &camel_obj_name,
        &messages.events,
        RequestHandlerDirection::Event,
    )?;
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

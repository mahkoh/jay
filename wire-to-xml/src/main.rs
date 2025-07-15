#![allow(
    clippy::len_zero,
    clippy::needless_lifetimes,
    clippy::enum_variant_names,
    clippy::useless_format,
    clippy::redundant_clone,
    clippy::collapsible_if,
    clippy::unnecessary_to_owned,
    clippy::match_like_matches_macro,
    clippy::too_many_arguments,
    clippy::iter_skip_next,
    clippy::uninlined_format_args,
    clippy::manual_is_ascii_check,
    clippy::single_char_pattern
)]

use {
    crate::parser::{Type, parse_messages},
    clap::Parser,
    quick_xml::events::{BytesDecl, BytesText, Event},
    std::{io, os::unix::ffi::OsStrExt, path::PathBuf},
};

#[path = "../../build/wire/parser.rs"]
#[allow(dead_code)]
mod parser;

#[derive(Parser, Debug)]
struct Cli {
    protocol: String,
    files: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut writer = quick_xml::Writer::new_with_indent(io::stdout().lock(), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer
        .create_element("protocol")
        .with_attribute(("name", &*cli.protocol))
        .write_inner_content(|w| {
            w.create_element("copyright").write_inner_content(|w| {
                for line in COPYRIGHT.lines() {
                    w.write_indent()?;
                    w.write_event(Event::Text(BytesText::new(line)))?;
                }
                Ok(())
            })?;
            w.create_element("description")
                .with_attribute(("summary", ""))
                .write_empty()?;
            for f in &cli.files {
                let res = parse_messages(std::fs::read(f)?.as_slice()).map_err(io::Error::other)?;
                let if_name = f.file_stem().unwrap();
                let version = res
                    .events
                    .iter()
                    .chain(res.requests.iter())
                    .map(|e| e.val.attribs.since.unwrap_or(1))
                    .max()
                    .unwrap_or(1);
                w.create_element("interface")
                    .with_attribute((&b"name"[..], if_name.as_bytes()))
                    .with_attribute(("version", &*version.to_string()))
                    .write_inner_content(|w| {
                        w.create_element("description")
                            .with_attribute(("summary", ""))
                            .write_empty()?;
                        for (ty, messages) in [("request", &res.requests), ("event", &res.events)] {
                            for message in messages {
                                let mut el = w
                                    .create_element(ty)
                                    .with_attribute(("name", &*message.val.name));
                                if let Some(since) = message.val.attribs.since {
                                    el = el.with_attribute(("since", &*since.to_string()));
                                }
                                if message.val.attribs.destructor {
                                    el = el.with_attribute(("type", "destructor"));
                                }
                                el.write_inner_content(|w| {
                                    w.create_element("description")
                                        .with_attribute(("summary", ""))
                                        .write_empty()?;
                                    let mut i = 0;
                                    while i < message.val.fields.len() {
                                        let j = i + 2;
                                        if j < message.val.fields.len() {
                                            if let Type::Id(name, _) =
                                                &message.val.fields[j].val.ty.val
                                            {
                                                if name == "object" {
                                                    i = j;
                                                }
                                            }
                                        }
                                        let field = &message.val.fields[i];
                                        let mut el = w.create_element("arg");
                                        macro_rules! simple {
                                            ($ty:expr) => {
                                                el = el
                                                    .with_attribute(("name", &*field.val.name))
                                                    .with_attribute(("type", $ty));
                                            };
                                        }
                                        match &field.val.ty.val {
                                            Type::Id(name, _) => {
                                                let ty = match field.val.attribs.new {
                                                    true => "new_id",
                                                    false => "object",
                                                };
                                                simple!(ty);
                                                if name != "object" {
                                                    el = el.with_attribute(("interface", &**name));
                                                }
                                                if field.val.attribs.nullable {
                                                    el = el.with_attribute(("allow-null", "true"));
                                                }
                                            }
                                            Type::U32 => {
                                                simple!("uint");
                                            }
                                            Type::I32 => {
                                                simple!("int");
                                            }
                                            t @ Type::U64 | t @ Type::U64Rev => {
                                                let mut suf = ["hi", "lo"];
                                                if let Type::U64Rev = t {
                                                    suf = ["lo", "hi"];
                                                }
                                                el.with_attribute((
                                                    "name",
                                                    &*format!("{}_{}", field.val.name, suf[0]),
                                                ))
                                                .with_attribute(("type", "uint"))
                                                .with_attribute(("description", ""))
                                                .write_empty()?;
                                                el = w
                                                    .create_element("arg")
                                                    .with_attribute((
                                                        "name",
                                                        &*format!("{}_{}", field.val.name, suf[1]),
                                                    ))
                                                    .with_attribute(("type", "uint"));
                                            }
                                            Type::Str | Type::BStr => {
                                                simple!("string");
                                            }
                                            Type::OptStr => {
                                                simple!("string");
                                                el = el.with_attribute(("allow-null", "true"));
                                            }
                                            Type::Fixed => {
                                                simple!("fixed");
                                            }
                                            Type::Fd => {
                                                simple!("fd");
                                            }
                                            Type::Array(_) | Type::Pod(_) => {
                                                simple!("array");
                                            }
                                        }
                                        el.with_attribute(("description", "")).write_empty()?;
                                        i += 1;
                                    }
                                    Ok(())
                                })?;
                            }
                        }
                        Ok(())
                    })?;
            }
            Ok(())
        })?;
    Ok(())
}

const COPYRIGHT: &str = r#"Copyright 20XX YY

Permission is hereby granted, free of charge, to any person obtaining a
copy of this software and associated documentation files (the "Software"),
to deal in the Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice (including the next
paragraph) shall be included in all copies or substantial portions of the
Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
"#;

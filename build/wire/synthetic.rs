use crate::open;
use crate::wire::ParsedFile;
use crate::wire::parser::Type;
use std::env;
use std::fmt;
use std::fmt::Formatter;
use std::io::Write;
use std::mem;
use std::path::Path;

pub fn write_synthetic_helpers(files: &[ParsedFile]) -> anyhow::Result<()> {
    std::fs::create_dir_all(Path::new(&env::var("OUT_DIR").unwrap()).join("synthetic_helpers"))?;
    let mut f = open("synthetic_helpers/mod.rs")?;
    define_w!(f, w, wl);
    define_xn!(xn);
    for file in files {
        write_synthetic_helper(file)?;
        let name = &file.obj_name.raw();
        if !file.messages.synthetics {
            wl!(r#"{xn}#[cfg(feature = "it")]"#);
        }
        wl!("{xn}mod {name};")
    }
    Ok(())
}

fn write_synthetic_helper(file: &ParsedFile) -> anyhow::Result<()> {
    let obj_name = file.obj_name.raw();
    let camel_obj_name = &file.camel_obj_name;
    let singleton = file.messages.singleton;
    let mut f = open(&format!("synthetic_helpers/{obj_name}.rs"))?;
    define_w!(f, w, wl);
    define_xn!(xn);
    wl!("{xn}#![allow(dead_code)]");
    wl!("{xn}#![allow(unused_imports)]");
    wl!("{xn}#![allow(unused_parens)]");
    wl!("{xn}#![allow(clippy::unused_unit)]");
    wl!();
    wl!("{xn}use std::rc::Rc;");
    wl!("{xn}use uapi::OwnedFd;");
    wl!("{xn}use crate::wire;");
    wl!("{xn}use crate::client::Client;");
    wl!("{xn}use crate::globals::Singleton;");
    wl!();
    wl!("{xn}impl Client {{");
    {
        push_xn!(xn);
        for request in &file.messages.requests {
            let req = &request.val;
            let name = req.name.raw();
            let camel_name = &req.camel_name;
            wl!("{xn}pub(crate) fn send_{obj_name}_{name}(");
            {
                push_xn!(xn);
                wl!("{xn}self: &Rc<Self>,");
                if !singleton && obj_name != "wl_display" {
                    wl!("{xn}self_id: wire::{camel_obj_name}Id,");
                }
                for field in &req.fields {
                    let field = &field.val;
                    if let Type::Id(..) = &field.ty.val
                        && field.attribs.new
                    {
                        continue;
                    }
                    let name = field.name;
                    fn write_ty(f: &mut Formatter, ty: &Type) -> fmt::Result {
                        let ty = match ty {
                            Type::Id(_, camel) => {
                                write!(f, "wire::{camel}Id")?;
                                return Ok(());
                            }
                            Type::U32 => "u32",
                            Type::I32 => "i32",
                            Type::U64 => "u64",
                            Type::U64Rev => "u64",
                            Type::Str => "&str",
                            Type::OptStr => "Option<&str>",
                            Type::BStr => "&str",
                            Type::Fixed => "crate::fixed::Fixed",
                            Type::Fd => "&Rc<OwnedFd>",
                            Type::Bool => "bool",
                            Type::Array(v) => {
                                f.write_str("&[")?;
                                write_ty(f, v)?;
                                f.write_str("]")?;
                                return Ok(());
                            }
                            Type::Pod(v) => v,
                        };
                        f.write_str(ty)
                    }
                    let ty = fmt::from_fn(|f| write_ty(f, &field.ty.val));
                    wl!("{xn}{name}: {ty},");
                }
            }
            w!("{xn}) -> (");
            let mut first = true;
            for field in &req.fields {
                let field = &field.val;
                if let Type::Id(_, camel) = &field.ty.val
                    && field.attribs.new
                {
                    if !mem::take(&mut first) {
                        w!(", ");
                    }
                    w!("wire::{camel}Id");
                }
            }
            wl!(") {{");
            {
                push_xn!(xn);
                for field in &req.fields {
                    let field = &field.val;
                    if let Type::Id(..) = &field.ty.val
                        && field.attribs.new
                    {
                        let name = field.name;
                        wl!("{xn}let {name} = self.new_synthetic_id();");
                    }
                }
                wl!("{xn}self.request(wire::{obj_name}::{camel_name} {{");
                {
                    push_xn!(xn);
                    if obj_name == "wl_display" {
                        wl!("{xn}self_id: crate::object::WL_DISPLAY_ID,");
                    } else if singleton {
                        wl!(
                            "{xn}self_id: self.get_synthetic_singleton(Singleton::{camel_obj_name}),"
                        );
                    } else {
                        wl!("{xn}self_id,");
                    }
                    for field in &req.fields {
                        let field = &field.val;
                        let name = field.name;
                        if let Type::Fd = field.ty.val {
                            wl!("{xn}{name}: {name}.clone(),");
                        } else {
                            wl!("{xn}{name},");
                        }
                    }
                }
                wl!("{xn}}});");
                w!("{xn}(");
                let mut first = true;
                for field in &req.fields {
                    let field = &field.val;
                    if let Type::Id(..) = &field.ty.val
                        && field.attribs.new
                    {
                        if !mem::take(&mut first) {
                            w!(", ");
                        }
                        let name = field.name;
                        w!("{name}");
                    }
                }
                wl!(")");
            }
            wl!("{xn}}}");
        }
    }
    wl!("{xn}}}");
    Ok(())
}

use crate::open;
use crate::wire::ParsedFile;
use crate::wire::parser::Type;
use indexmap::IndexSet;
use std::env;
use std::fmt;
use std::io::Write;
use std::path::Path;

fn write_client_trace_file(
    arg_ids: &mut usize,
    arg_ranges: &mut Vec<(usize, usize)>,
    arrays: &mut IndexSet<String>,
    pods: &mut IndexSet<String>,
    file: &ParsedFile,
) -> anyhow::Result<()> {
    let ParsedFile {
        obj_name, messages, ..
    } = file;
    let f = &mut open(&format!("client_trace/{}.rs", obj_name.raw()))?;
    define_w!(f, w, wl);
    define_xn!(xn);
    wl!("use super::*;");
    for message in messages.requests.iter().chain(messages.events.iter()) {
        let safe_name = &message.val.safe_name;
        let camel_name = &message.val.camel_name;
        wl!();
        let lt = if message.val.has_reference_type {
            "<'_>"
        } else {
            ""
        };
        wl!(
            "impl ClientTraceMessagePriv for crate::wire::{}::{camel_name}{lt} {{",
            obj_name.raw(),
        );
        {
            push_xn!(xn);
            wl!(
                "{xn}fn write(&self, id: &mut u32, data: &mut [u32; MAX_MESSAGE_WORDS]) -> Option<usize> {{"
            );
            {
                push_xn!(xn);
                wl!(
                    "{xn}*id = def_indices::{}Ξ{} as u32;",
                    file.obj_name.raw(),
                    message.val.name.raw(),
                );
                let mut idx = 0;
                for field in &message.val.fields {
                    let name = field.val.name;
                    let prefix = |idx| {
                        fmt::from_fn(move |f| {
                            define_w!(f, w2, wl2);
                            w2!("{xn}data[{idx}] = ");
                            Ok(())
                        })
                    };
                    let idx_pfx = prefix(idx);
                    match &field.val.ty.val {
                        Type::Id(_, _) => wl!("{idx_pfx}self.{name}.raw() as u32;"),
                        Type::U32 => wl!("{idx_pfx}self.{name};"),
                        Type::I32 => wl!("{idx_pfx}self.{name} as u32;"),
                        Type::U64 | Type::U64Rev => {
                            wl!("{idx_pfx}self.{name} as u32;");
                            idx += 1;
                            let idx_pfx = prefix(idx);
                            wl!("{idx_pfx}(self.{name} >> 32) as u32;");
                        }
                        Type::Str | Type::BStr => {
                            wl!("{idx_pfx}self.{name}.len() as u32;");
                        }
                        Type::OptStr => {
                            wl!("{idx_pfx}opt_str_len(self.{name});");
                        }
                        Type::Fixed => {
                            wl!("{idx_pfx}self.{name}.0 as u32;");
                        }
                        Type::Fd => continue,
                        Type::Array(_) => {
                            wl!("{idx_pfx}self.{name}.len() as u32;");
                        }
                        Type::Pod(_) => continue,
                        Type::Bool => wl!("{idx_pfx}self.{name} as u32;"),
                    }
                    idx += 1;
                }
                if message.val.is_fixed_size {
                    let uses_data = message
                        .val
                        .fields
                        .iter()
                        .any(|f| !matches!(f.val.ty.val, Type::Fd));
                    if !uses_data {
                        wl!("{xn}let _ = data;");
                    }
                    wl!("{xn}Some({idx})");
                } else {
                    wl!("{xn}write_tail(");
                    {
                        push_xn!(xn);
                        wl!("{xn}data,");
                        wl!("{xn}{idx},");
                        wl!("{xn}[");
                        {
                            push_xn!(xn);
                            for field in &message.val.fields {
                                let name = &field.val.name;
                                match &field.val.ty.val {
                                    Type::Str | Type::BStr => {
                                        wl!("{xn}self.{name}.as_bytes(),");
                                    }
                                    Type::OptStr => {
                                        wl!("{xn}opt_str_bytes(self.{name}),");
                                    }
                                    Type::Array(_) => {
                                        wl!("{xn}uapi::as_bytes(self.{name}),");
                                    }
                                    Type::Pod(_) => {
                                        wl!("{xn}uapi::as_bytes(&self.{name}),");
                                    }
                                    _ => {}
                                }
                            }
                        }
                        wl!("{xn}],");
                    }
                    wl!("{xn})");
                }
            }
            wl!("{xn}}}");
        }
        wl!("}}");
        wl!();
        wl!("pub unsafe fn read_{safe_name}<'a, 'b>(");
        {
            push_xn!(xn);
            if message.val.is_fixed_size {
                wl!("{xn}data: *mut u32,");
            } else {
                wl!("{xn}mut data: *mut u32,");
            }
            wl!("{xn}vals: &'b mut [MaybeUninit<ClientTraceArg<'a>>; MAX_ARGS],");
        }
        wl!(") {{");
        {
            push_xn!(xn);
            let has_args = message.val.fields.len() > 0;
            let mut has_fixed_args = false;
            for field in &message.val.fields {
                match &field.val.ty.val {
                    Type::Id(_, _)
                    | Type::U32
                    | Type::I32
                    | Type::Str
                    | Type::BStr
                    | Type::OptStr
                    | Type::Fixed
                    | Type::Array(_)
                    | Type::U64
                    | Type::U64Rev
                    | Type::Bool => {
                        has_fixed_args = true;
                        break;
                    }
                    Type::Pod(_) | Type::Fd => {}
                }
            }
            let first_arg_id = *arg_ids;
            let last_arg_id = first_arg_id + message.val.fields.len();
            *arg_ids = last_arg_id;
            arg_ranges.push((first_arg_id, last_arg_id));
            if has_args {
                wl!("{xn}unsafe {{");
                {
                    push_xn!(xn);
                    if has_fixed_args {
                        let mut num = 0;
                        wl!("{xn}let [");
                        {
                            push_xn!(xn);
                            for (idx, field) in message.val.fields.iter().enumerate() {
                                match &field.val.ty.val {
                                    Type::Id(_, _)
                                    | Type::U32
                                    | Type::I32
                                    | Type::Str
                                    | Type::BStr
                                    | Type::OptStr
                                    | Type::Fixed
                                    | Type::Bool
                                    | Type::Array(_) => {
                                        wl!("{xn}arg{idx},");
                                        num += 1;
                                    }
                                    Type::Pod(_) => continue,
                                    Type::Fd => continue,
                                    Type::U64 | Type::U64Rev => {
                                        wl!("{xn}arg{idx}_lo,");
                                        wl!("{xn}arg{idx}_hi,");
                                        num += 2;
                                    }
                                }
                            }
                        }
                        wl!("{xn}] = (data as *mut [u32; {num}]).read();");
                        if !message.val.is_fixed_size {
                            wl!("{xn}data = data.add({num});");
                        }
                    }
                    for (idx, field) in message.val.fields.iter().enumerate() {
                        let pfx = fmt::from_fn(|f| {
                            define_w!(f, w2, wl2);
                            w2!(
                                "{xn}(&raw mut (*vals[{idx}].as_mut_ptr()).val).write(ClientTraceArgVal::"
                            );
                            Ok(())
                        });
                        match &field.val.ty.val {
                            Type::Id(_, _) => {
                                wl!("{pfx}Id(arg{idx} as u64));");
                            }
                            Type::U32 => wl!("{pfx}U32(arg{idx}));"),
                            Type::Bool => wl!("{pfx}Bool(arg{idx} != 0));"),
                            Type::I32 => wl!("{pfx}I32(arg{idx} as i32));"),
                            Type::U64 | Type::U64Rev => {
                                wl!("{pfx}U64((arg{idx}_hi as u64) << 32 | arg{idx}_lo as u64));")
                            }
                            Type::Fixed => {
                                wl!("{pfx}Fixed(crate::fixed::Fixed(arg{idx} as i32)));")
                            }
                            Type::Fd => wl!("{pfx}Fd);"),
                            Type::OptStr => {
                                wl!(r#"{xn}let mut slice = None;"#);
                                wl!(r#"{xn}if arg{idx} > 0 {{"#);
                                {
                                    push_xn!(xn);
                                    wl!(
                                        r#"{xn}slice = Some(slice::from_raw_parts(data.cast(), arg{idx} as usize - 1));"#
                                    );
                                }
                                wl!(r#"{xn}}}"#);
                                wl!(r#"{pfx}Str(slice));"#);
                                wl!(
                                    r#"{xn}data = data.add((arg{idx} as usize + WORD_SIZE - 2) / WORD_SIZE);"#
                                );
                            }
                            Type::Str | Type::BStr => {
                                wl!(
                                    r#"{pfx}Str(Some(slice::from_raw_parts(data.cast(), arg{idx} as usize))));"#
                                );
                                wl!(
                                    r#"{xn}data = data.add((arg{idx} as usize + WORD_SIZE - 1) / WORD_SIZE);"#
                                );
                            }
                            Type::Array(n) => {
                                let ty = n.array_ty_name();
                                let v = match arrays.get_index_of(ty) {
                                    Some(idx) => idx,
                                    None => {
                                        let idx = arrays.len();
                                        arrays.insert(ty.to_string());
                                        idx
                                    }
                                };
                                wl!(
                                    r#"{pfx}Array(ClientTraceArray::V{v}(slice::from_raw_parts(data.cast(), arg{idx} as usize))));"#
                                );
                                wl!(
                                    r#"{xn}data = data.add(((arg{idx} as usize * size_of::<{ty}>()) + WORD_SIZE - 1) / WORD_SIZE);"#
                                );
                            }
                            Type::Pod(ty) => {
                                let v = match pods.get_index_of(ty) {
                                    Some(idx) => idx,
                                    None => {
                                        let idx = pods.len();
                                        pods.insert(ty.clone());
                                        idx
                                    }
                                };
                                wl!(
                                    r#"{pfx}Pod(ClientTracePod::V{v}(read_unaligned(data.cast()))));"#
                                );
                                wl!(
                                    r#"{xn}data = data.add((size_of::<{ty}>() + WORD_SIZE - 1) / WORD_SIZE);"#
                                );
                            }
                        }
                    }
                    wl!("{xn}let _ = data;");
                }
                wl!("{xn}}}");
            } else {
                wl!("{xn}let _ = data;");
                wl!("{xn}let _ = vals;");
            }
        }
        wl!("}}");
    }
    Ok(())
}

pub fn write_client_trace_files(files: &[ParsedFile]) -> anyhow::Result<()> {
    std::fs::create_dir_all(Path::new(&env::var("OUT_DIR").unwrap()).join("client_trace"))?;
    let mut arg_ids = 0;
    let mut arg_ranges = vec![];
    let mut arrays = IndexSet::new();
    let mut pods = IndexSet::new();
    for file in files {
        write_client_trace_file(&mut arg_ids, &mut arg_ranges, &mut arrays, &mut pods, file)?;
    }
    let mut f = open("client_trace/mod.rs")?;
    define_w!(f, w, wl);
    define_xn!(xn);
    wl!("use super::ClientTraceMessageDef;");
    wl!("use super::ClientTraceMessagePriv;");
    wl!("use super::ClientTraceArgDef;");
    wl!("use super::ClientTraceArg;");
    wl!("use super::ClientTraceArgVal;");
    wl!("use super::MAX_MESSAGE_WORDS;");
    wl!("use super::WORD_SIZE;");
    wl!("use super::helpers::opt_str_len;");
    wl!("use super::helpers::opt_str_bytes;");
    wl!("use super::helpers::write_tail;");
    wl!("use super::helpers::read_unaligned;");
    wl!("use super::Reader;");
    wl!("use bstr::ByteSlice;");
    wl!("use crate::utils::str_fmt::StrFmt;");
    wl!("use crate::utils::str_fmt::StrCtx;");
    wl!("use std::mem::MaybeUninit;");
    wl!("use std::slice;");
    wl!("use crate::utils::str_table::StrAccess;");
    wl!();
    for file in files {
        wl!("mod {};", file.obj_name.raw());
    }
    wl!();
    let mut num_readers = 0;
    let mut max_args = 0;
    for file in files {
        for message in file.messages.messages() {
            num_readers += 1;
            max_args = max_args.max(message.val.fields.len());
        }
    }
    wl!("pub const MAX_ARGS: usize = {max_args};");
    wl!();
    wl!("pub static READERS: [Reader; {num_readers}] = [");
    {
        push_xn!(xn);
        for file in files {
            for message in file.messages.messages() {
                wl!(
                    "{xn}{}::read_{},",
                    file.obj_name.raw(),
                    message.val.safe_name
                );
            }
        }
    }
    wl!("];");
    wl!();
    wl!("#[derive(Copy, Clone)]");
    wl!("pub enum ClientTraceArray<'a> {{");
    {
        push_xn!(xn);
        for (idx, ty) in arrays.iter().enumerate() {
            wl!("{xn}V{idx}(&'a [{ty}]),");
        }
    }
    wl!("}}");
    wl!();
    for ty in &arrays {
        wl!("static_assertions::const_assert!(align_of::<{ty}>() <= WORD_SIZE);");
        wl!("static_assertions::assert_impl_all!({ty}: uapi::Pod, uapi::Packed);");
    }
    wl!();
    wl!("impl StrFmt for ClientTraceArray<'_> {{");
    {
        push_xn!(xn);
        wl!("{xn}fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {{",);
        {
            push_xn!(xn);
            wl!("{xn}match *self {{");
            {
                push_xn!(xn);
                for (idx, _) in arrays.iter().enumerate() {
                    wl!("{xn}Self::V{idx}(n) => n.str_fmt(dst, ctx),");
                }
            }
            wl!("{xn}}}");
        }
        wl!("{xn}}}");
    }
    wl!("}}");
    wl!();
    wl!("#[derive(Copy, Clone)]");
    wl!("pub enum ClientTracePod {{");
    {
        push_xn!(xn);
        for (idx, ty) in pods.iter().enumerate() {
            wl!("{xn}V{idx}({ty}),");
        }
    }
    wl!("}}");
    wl!();
    for ty in &pods {
        wl!("static_assertions::assert_impl_all!({ty}: uapi::Pod, uapi::Packed);");
    }
    wl!();
    wl!("impl StrFmt for ClientTracePod {{");
    {
        push_xn!(xn);
        wl!("{xn}fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {{",);
        {
            push_xn!(xn);
            wl!("{xn}match self {{");
            {
                push_xn!(xn);
                for (idx, _) in pods.iter().enumerate() {
                    wl!("{xn}Self::V{idx}(n) => n.str_fmt(dst, ctx),");
                }
            }
            wl!("{xn}}}");
        }
        wl!("{xn}}}");
    }
    wl!("}}");
    wl!();
    wl!(r#"pub static DEFS: [ClientTraceMessageDef; {num_readers}] = ["#);
    {
        push_xn!(xn);
        let mut idx = 0;
        for file in files {
            for msg in file.messages.messages() {
                let has_ids = msg
                    .val
                    .fields
                    .iter()
                    .any(|f| matches!(f.val.ty.val, Type::Id(..)));
                wl!(r#"{xn}ClientTraceMessageDef {{"#);
                {
                    push_xn!(xn);
                    wl!(r#"{xn}is_request: {},"#, msg.val.is_request);
                    wl!(r#"{xn}has_ids: {has_ids},"#);
                    wl!(r#"{xn}interface: {},"#, file.obj_name);
                    wl!(r#"{xn}message: {},"#, msg.val.name);
                    wl!(r#"{xn}args: {:?},"#, arg_ranges[idx]);
                }
                wl!(r#"{xn}}},"#);
                idx += 1;
            }
        }
    }
    wl!(r#"];"#);
    wl!();
    wl!(r#"pub static ARGS: [ClientTraceArgDef; {arg_ids}] = ["#);
    {
        push_xn!(xn);
        for file in files {
            for msg in file.messages.messages() {
                for field in &msg.val.fields {
                    wl!(r#"{xn}ClientTraceArgDef {{"#);
                    {
                        push_xn!(xn);
                        wl!(r#"{xn}name: {},"#, field.val.name_interned);
                        wl!(
                            r#"{xn}interface: {},"#,
                            fmt::from_fn(|f| {
                                define_w!(f, w2, wl2);
                                match &field.val.ty.val {
                                    Type::Id(n, _) if n.raw() != "object" => w2!(r#"Some({n})"#),
                                    _ => w2!("None"),
                                }
                                Ok(())
                            })
                        );
                    }
                    wl!(r#"{xn}}},"#);
                }
            }
        }
    }
    wl!(r#"];"#);
    wl!();
    wl!(r#"#[allow(clippy::allow_attributes, non_upper_case_globals)]"#);
    wl!(r#"mod def_indices {{"#);
    {
        push_xn!(xn);
        let mut idx = 0;
        for file in files {
            for msg in file.messages.messages() {
                wl!(
                    r#"{xn}pub static {}Ξ{}: usize = {idx};"#,
                    file.obj_name.raw(),
                    msg.val.name.raw()
                );
                idx += 1;
            }
        }
    }
    wl!(r#"}}"#);
    Ok(())
}

impl Type {
    fn array_ty_name(&self) -> &str {
        match self {
            Type::Id(_, _) => "u32",
            Type::U32 => "u32",
            Type::I32 => "i32",
            Type::U64 | Type::U64Rev => "u64",
            Type::Fixed => "crate::fixed::Fixed",
            Type::Pod(n) => n,
            _ => unreachable!(),
        }
    }
}

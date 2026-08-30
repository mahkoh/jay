use crate::fuse::generated::TARGETS;
use crate::open;
use anyhow::Result;
use isnt::std_1::ops::IsntRangeExt;
use linearize::Linearize;
use linearize::StaticMap;
use std::env;
use std::fmt;
use std::fmt::Display;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;

mod generated;

#[derive(Copy, Clone)]
struct Target {
    path: &'static str,
    dirs: &'static [Dir],
    is_global: bool,
}

#[derive(Copy, Clone)]
struct Dir {
    name: &'static str,
    abstract_: bool,
    dirents: &'static [Ent],
    parents: &'static [&'static str],
    phf: PhfMap,
}

#[derive(Copy, Clone)]
struct Ent {
    name: &'static str,
    camel: &'static str,
    ty: EntTy,
    opt: bool,
    other: bool,
    inherited: bool,
    no_timeout: bool,
    predefined_key: Option<u32>,
}

#[derive(Copy, Clone, PartialEq, Linearize)]
enum EntTy {
    Reg,
    Link,
    View,
    Custom,
}

impl Ent {
    fn use_has(&self) -> bool {
        self.opt
            && (matches!(self.ty, EntTy::Reg | EntTy::Link)
                || (self.ty == EntTy::View && !self.other))
    }

    fn timeout(&self) -> impl Display {
        fmt::from_fn(move |f| {
            let s = match self.no_timeout() {
                true => "FUSE_NO_TIMEOUT",
                false => "FUSE_SHORT_TIMEOUT",
            };
            f.write_str(s)
        })
    }

    fn no_timeout(&self) -> bool {
        if self.no_timeout {
            return true;
        }
        if self.opt {
            return false;
        }
        match self.ty {
            EntTy::Reg => true,
            EntTy::Link => true,
            EntTy::View if self.other => false,
            EntTy::View if self.predefined_key.is_none() => false,
            EntTy::View => true,
            EntTy::Custom => false,
        }
    }

    fn short_timeout(&self) -> bool {
        !self.no_timeout()
    }
}

const CHILD_ENT_TY: [EntTy; 2] = [EntTy::Reg, EntTy::Link];

#[derive(Copy, Clone)]
struct PhfMap {
    key: u64,
    disps: &'static [(u32, u32)],
    map: &'static [usize],
}

pub fn main() -> Result<()> {
    std::fs::create_dir_all(Path::new(&env::var("OUT_DIR").unwrap()).join("fuse"))?;
    for target in TARGETS {
        generate(target)?;
    }
    Ok(())
}

struct ChildEnt<'a> {
    ent: &'a Ent,
    key: usize,
}

impl Deref for ChildEnt<'_> {
    type Target = Ent;

    fn deref(&self) -> &Self::Target {
        self.ent
    }
}

fn generate(target: &Target) -> Result<()> {
    let mut f = open(target.path)?;
    define_w!(f, w, wl);
    define_xn!(xn);
    wl!("{xn}mod generated {{");
    {
        push_xn!(xn);
        wl!("{xn}#![allow(unused_imports)]");
        wl!();
        wl!("{xn}use crate::utils::fuse::fuse_dir::FuseDirents;");
        wl!("{xn}use crate::utils::fuse::fuse_dir::FuseDirentName;");
        wl!("{xn}use crate::utils::fuse::fuse_dir::FUSE_NO_TIMEOUT;");
        wl!("{xn}use crate::utils::fuse::fuse_dir::FUSE_SHORT_TIMEOUT;");
        wl!("{xn}use crate::utils::fuse::fuse_inode::FuseInode;");
        wl!("{xn}use crate::utils::fuse::fuse_dir::FuseDirent;");
        wl!("{xn}use crate::utils::fuse::fuse_inode::FuseInodeWithKey;");
        wl!("{xn}use crate::utils::fuse::fuse_inode::FuseInodeProps;");
        wl!("{xn}use crate::utils::fuse::fuse_view::FuseView;");
        wl!("{xn}use jay_toml_config::phf_map::PhfMap;");
        wl!("{xn}use crate::utils::str_fmt::StrCtx;");
        wl!("{xn}use crate::utils::str_fmt::StrFmtFmt;");
        wl!("{xn}use crate::utils::type_view::TypeViewExt1;");
        wl!("{xn}use crate::utils::liveness::GetLiveness;");
        wl!("{xn}use std::rc::Rc;");
        if !target.is_global {
            wl!("{xn}use crate::utils::fuse::fuse_globals::*;");
        }
        for dir in target.dirs {
            let dirents = dir.dirents;
            let name = dir.name;
            let mut dirents: Vec<_> = dirents.iter().map(|ent| ChildEnt { ent, key: 0 }).collect();
            let mut child_dirents_range = StaticMap::default();
            let mut hi = 0;
            for ty in CHILD_ENT_TY {
                let lo = hi;
                for ent in &mut dirents {
                    if ent.ty == ty {
                        ent.key = hi;
                        hi += 1;
                    }
                }
                child_dirents_range[ty] = lo..hi;
            }
            let has_child_dirents = hi > 0;
            let dirents = &dirents;
            wl!();
            wl!("{xn}pub mod {name} {{");
            {
                push_xn!(xn);
                let parents = fmt::from_fn(|f| {
                    for n in dir.parents {
                        write!(f, " + super::{n}::Dir")?;
                    }
                    Ok(())
                });
                wl!("{xn}use super::*;");
                let vis = if target.is_global {
                    "crate"
                } else {
                    "in super::super"
                };
                wl!("{xn}pub({vis}) trait Dir: GetLiveness{parents} + 'static {{");
                {
                    push_xn!(xn);
                    for de in dirents {
                        if de.inherited {
                            continue;
                        }
                        let camel = &de.camel;
                        if de.ty == EntTy::View {
                            w!("{xn}type View{camel}: FuseView<Self");
                            if de.other {
                                wl!("::Base{camel}>;");
                                wl!("{xn}type Base{camel}: GetLiveness;");
                            } else {
                                wl!(">;");
                            }
                        }
                    }
                    for de in dirents {
                        if de.inherited {
                            continue;
                        }
                        let name = de.name;
                        let camel = de.camel;
                        match de.ty {
                            EntTy::Reg => {
                                if de.use_has() {
                                    wl!("{xn}fn has_{name}(&self) -> bool;");
                                }
                                wl!("{xn}fn read_{name}(&self, buf: &mut String, ctx: &StrCtx);");
                            }
                            EntTy::Link => {
                                if de.use_has() {
                                    wl!("{xn}fn has_{name}(&self) -> bool;");
                                }
                                wl!("{xn}fn readlink_{name}(&self, depth: u64, buf: &mut String);");
                            }
                            EntTy::View => {
                                if de.use_has() {
                                    wl!("{xn}fn has_{name}(&self, key: u64) -> bool;");
                                }
                                if de.predefined_key.is_none() {
                                    wl!("{xn}fn keyof_{name}(&self, key: u64) -> u64;");
                                }
                                if de.other {
                                    w!("{xn}fn get_{name}(self: &Rc<Self>, key: u64) -> ");
                                    if de.opt {
                                        w!("Option<");
                                    }
                                    w!("Rc<Self::Base{camel}>");
                                    if de.opt {
                                        w!(">");
                                    }
                                    wl!(";");
                                }
                            }
                            EntTy::Custom => {
                                w!("{xn}fn get_{name}(self: &Rc<Self>, key: u64) -> ");
                                if de.opt {
                                    w!("Option<");
                                }
                                if de.predefined_key.is_some() {
                                    w!("Rc<dyn FuseInode>");
                                } else {
                                    w!("FuseInodeWithKey");
                                }
                                if de.opt {
                                    w!(">");
                                }
                                wl!(";");
                            }
                        }
                    }
                }
                wl!("{xn}}}");
                if !dir.abstract_ {
                    if !dirents.is_empty() {
                        wl!("{xn}#[derive(Copy, Clone)]");
                        wl!("{xn}enum File {{");
                        {
                            push_xn!(xn);
                            for de in dirents {
                                let camel = de.camel;
                                wl!("{xn}{camel},");
                            }
                        }
                        wl!("{xn}}}");
                        let phf = dir.phf;
                        wl!("{xn}static FILES: PhfMap<str, (&'static str, File)> = PhfMap {{");
                        {
                            push_xn!(xn);
                            wl!("{xn}key: {},", phf.key);
                            wl!("{xn}disps: &[");
                            {
                                push_xn!(xn);
                                for dis in phf.disps {
                                    wl!("{xn}{dis:?},");
                                }
                            }
                            wl!("{xn}],");
                            wl!("{xn}map: &[");
                            {
                                push_xn!(xn);
                                for &idx in phf.map {
                                    let val = &dirents[idx];
                                    let name = val.name;
                                    let camel = val.camel;
                                    wl!("{xn}({name:?}, File::{camel}),");
                                }
                            }
                            wl!("{xn}],");
                            wl!("{xn}_phantom: core::marker::PhantomData,");
                        }
                        wl!("{xn}}};");
                    }
                    wl!("{xn}pub struct View;");
                    wl!("{xn}impl<T> FuseView<T> for View");
                    {
                        push_xn!(xn);
                        wl!("{xn}where T: Dir,");
                    }
                    wl!("{xn}{{");
                    {
                        push_xn!(xn);
                        wl!("{xn}fn props(_t: &T, _key: u64) -> FuseInodeProps {{");
                        {
                            push_xn!(xn);
                            wl!("{xn}FuseInodeProps::dir()");
                        }
                        wl!("{xn}}}");
                        wl!(
                            "{xn}fn lookup(t: Rc<T>, key: u64, name: &str) -> Option<FuseDirent> {{"
                        );
                        {
                            push_xn!(xn);
                            wl!("{xn}let _ = key;");
                            if dirents.is_empty() {
                                wl!("{xn}let _ = t;");
                                wl!("{xn}let _ = name;");
                                wl!("{xn}None");
                            } else {
                                wl!("{xn}let (actual, file) = FILES[name];");
                                wl!("{xn}if actual != name {{");
                                {
                                    push_xn!(xn);
                                    wl!("{xn}return None;");
                                }
                                wl!("{xn}}}");
                                if has_child_dirents {
                                    wl!("{xn}let name;");
                                    wl!("{xn}#[allow(unused_mut, unused_assignments)]");
                                    wl!("{xn}let mut timeout_ns = FUSE_NO_TIMEOUT;");
                                    wl!("{xn}let key = match file {{");
                                } else {
                                    wl!("{xn}match file {{");
                                }
                                {
                                    push_xn!(xn);
                                    for de in dirents {
                                        let name = de.name;
                                        let camel = de.camel;
                                        wl!("{xn}File::{camel} => {{");
                                        {
                                            push_xn!(xn);
                                            if de.use_has() {
                                                let key =
                                                    if de.ty == EntTy::View { "key" } else { "" };
                                                wl!("{xn}if !t.has_{name}({key}) {{");
                                                {
                                                    push_xn!(xn);
                                                    wl!("{xn}return None;");
                                                }
                                                wl!("{xn}}}");
                                            }
                                            match de.ty {
                                                EntTy::Reg | EntTy::Link => {
                                                    wl!("{xn}name = \"{name}\";");
                                                    if de.short_timeout() {
                                                        wl!("{xn}timeout_ns = FUSE_SHORT_TIMEOUT;");
                                                    }
                                                    wl!("{xn}{}", de.key);
                                                }
                                                EntTy::View => {
                                                    wl!("{xn}return Some(FuseDirent {{");
                                                    {
                                                        push_xn!(xn);
                                                        if let Some(key) = de.predefined_key {
                                                            wl!("{xn}key: {key},");
                                                        } else {
                                                            wl!("{xn}key: t.keyof_{name}(key),");
                                                        }
                                                        w!("{xn}inode: ");
                                                        if de.other {
                                                            w!("t.get_{name}(key)");
                                                            if de.opt {
                                                                w!("?");
                                                            }
                                                        } else {
                                                            w!("t");
                                                        }
                                                        wl!(".tv_wrap_rc::<T::View{camel}>(),");
                                                        wl!("{xn}static_name: Some(\"{name}\"),");
                                                        wl!("{xn}timeout_ns: {},", de.timeout());
                                                    }
                                                    wl!("{xn}}});");
                                                }
                                                EntTy::Custom => {
                                                    w!("{xn}let ");
                                                    if de.predefined_key.is_some() {
                                                        w!("inode");
                                                    } else {
                                                        w!("FuseInodeWithKey {{ inode, key }}");
                                                    }
                                                    w!(" = t.get_{name}(key)");
                                                    if de.opt {
                                                        w!("?");
                                                    }
                                                    wl!(";");
                                                    wl!("{xn}let f = FuseDirent {{");
                                                    {
                                                        push_xn!(xn);
                                                        wl!("{xn}inode,");
                                                        if let Some(key) = de.predefined_key {
                                                            wl!("{xn}key: {key},");
                                                        } else {
                                                            wl!("{xn}key,");
                                                        }
                                                        wl!("{xn}static_name: Some(\"{name}\"),");
                                                        wl!("{xn}timeout_ns: {},", de.timeout());
                                                    }
                                                    wl!("{xn}}};");
                                                    wl!("{xn}return Some(f);");
                                                }
                                            }
                                        }
                                        wl!("{xn}}}");
                                    }
                                }
                                wl!("{xn}}};");
                                if has_child_dirents {
                                    wl!("{xn}Some(FuseDirent {{");
                                    {
                                        push_xn!(xn);
                                        wl!("{xn}inode: t.tv_wrap_rc_ref_clone::<ChildView>(),");
                                        wl!("{xn}key,");
                                        wl!("{xn}static_name: Some(name),");
                                        wl!("{xn}timeout_ns,");
                                    }
                                    wl!("{xn}}})");
                                }
                            }
                        }
                        wl!("{xn}}}");
                        wl!("{xn}fn getdents(t: Rc<T>, key: u64, dirents: &mut FuseDirents) {{");
                        {
                            push_xn!(xn);
                            wl!("{xn}let _ = key;");
                            if has_child_dirents {
                                wl!("{xn}let child = t.tv_wrap_rc_ref::<ChildView>();");
                            }
                            for de in dirents {
                                let name = de.name;
                                let camel = de.camel;
                                let mut xn2 = xn.clone();
                                let timeout = de.timeout();
                                macro_rules! add {
                                    ($xn:expr, ($($inode:tt)*), ($($key:tt)*) $(,)?) => {{
                                        wl!("{}dirents.add({timeout}, {}, {}, FuseDirentName::Static(\"{name}\"));", $xn, format_args!($($inode)*), format_args!($($key)*))
                                    }};
                                }
                                match de.ty {
                                    EntTy::Reg | EntTy::Link => {
                                        if de.use_has() {
                                            wl!("{xn}if t.has_{name}() {{");
                                            xn2 = xn.push();
                                        }
                                        add!(&xn2, ("child"), ("{}", de.key));
                                    }
                                    EntTy::View => {
                                        if de.use_has() {
                                            wl!("{xn}if t.has_{name}(key) {{");
                                            xn2 = xn.push();
                                        }
                                        let key = fmt::from_fn(|f| match de.predefined_key {
                                            None => write!(f, "t.keyof_{name}(key)"),
                                            Some(key) => write!(f, "{key}"),
                                        });
                                        let t = fmt::from_fn(|f| match (de.opt, de.other) {
                                            (false, true) => write!(f, "t.get_{name}(key)"),
                                            (true, true) => f.write_str("u"),
                                            (_, false) => f.write_str("t"),
                                        });
                                        if de.opt && de.other {
                                            wl!("{xn}if let Some({t}) = t.get_{name}(key) {{");
                                            xn2 = xn.push();
                                        }
                                        add!(
                                            &xn2,
                                            ("{t}.tv_wrap_rc_ref::<T::View{camel}>()"),
                                            ("{key}"),
                                        );
                                        if de.opt && de.other {
                                            wl!("{xn}}}");
                                        }
                                    }
                                    EntTy::Custom => {
                                        if de.opt {
                                            wl!("{xn2}if let Some(f) = t.get_{name}(key) {{");
                                        } else {
                                            wl!("{xn2}{{");
                                        }
                                        {
                                            push_xn!(xn2);
                                            if !de.opt {
                                                wl!("{xn2}let f = t.get_{name}(key);");
                                            }
                                            if let Some(key) = de.predefined_key {
                                                wl!("{xn2}let f = f.with_key({key});");
                                            }
                                            wl!(
                                                "{xn2}dirents.add_dyn({timeout}, f, FuseDirentName::Static(\"{name}\"));"
                                            );
                                        }
                                        wl!("{xn2}}}");
                                    }
                                }
                                if de.use_has() {
                                    wl!("{xn}}}");
                                }
                            }
                        }
                        wl!("{xn}}}");
                    }
                    wl!("{xn}}}");
                    if has_child_dirents {
                        wl!("{xn}struct ChildView;");
                        wl!("{xn}impl<T> FuseView<T> for ChildView");
                        {
                            push_xn!(xn);
                            wl!("{xn}where T: Dir,");
                        }
                        wl!("{xn}{{");
                        {
                            push_xn!(xn);
                            wl!("{xn}fn props(_t: &T, key: u64) -> FuseInodeProps {{");
                            {
                                push_xn!(xn);
                                wl!("{xn}match key {{");
                                {
                                    push_xn!(xn);
                                    for ty in CHILD_ENT_TY {
                                        let range = &child_dirents_range[ty];
                                        if range.len() > 0 {
                                            let ty = match ty {
                                                EntTy::Reg => "reg",
                                                EntTy::Link => "link",
                                                EntTy::View | EntTy::Custom => unreachable!(),
                                            };
                                            wl!(
                                                "{xn}{}..{} => FuseInodeProps::{ty}(),",
                                                range.start,
                                                range.end
                                            );
                                        }
                                    }
                                    wl!("{xn}_ => FuseInodeProps::reg(),");
                                }
                                wl!("{xn}}}");
                            }
                            wl!("{xn}}}");
                            if child_dirents_range[EntTy::Reg].is_not_empty() {
                                wl!(
                                    "{xn}fn read(t: &T, key: u64, buf: &mut String, ctx: &StrCtx) {{"
                                );
                                {
                                    push_xn!(xn);
                                    wl!("{xn}match key {{");
                                    {
                                        push_xn!(xn);
                                        for ent in dirents {
                                            if ent.ty != EntTy::Reg {
                                                continue;
                                            }
                                            let key = ent.key;
                                            let name = ent.name;
                                            wl!("{xn}{key} => t.read_{name}(buf, ctx),");
                                        }
                                        wl!("{xn}_ => {{}},");
                                    }
                                    wl!("{xn}}}");
                                    wl!("{xn}if ctx.fmt == StrFmtFmt::Human {{");
                                    {
                                        push_xn!(xn);
                                        wl!("{xn}buf.push_str(\"\\n\");");
                                    }
                                    wl!("{xn}}}");
                                }
                                wl!("{xn}}}");
                            }
                            if child_dirents_range[EntTy::Link].is_not_empty() {
                                wl!(
                                    "{xn}fn readlink(t: &T, key: u64, depth: u64, buf: &mut String) {{"
                                );
                                {
                                    push_xn!(xn);
                                    wl!("{xn}match key {{");
                                    {
                                        push_xn!(xn);
                                        for ent in dirents {
                                            if ent.ty != EntTy::Link {
                                                continue;
                                            }
                                            wl!(
                                                "{xn}{} => t.readlink_{}(depth, buf),",
                                                ent.key,
                                                ent.name
                                            );
                                        }
                                        wl!("{xn}_ => {{}},");
                                    }
                                    wl!("{xn}}}");
                                }
                                wl!("{xn}}}");
                            }
                        }
                        wl!("{xn}}}");
                    }
                }
            }
            wl!("{xn}}}");
        }
    }
    wl!("{xn}}}");
    Ok(())
}

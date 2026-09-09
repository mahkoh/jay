use crate::open;
use crate::wire::ParsedFile;
use std::io::Write;

pub fn write_singletons(files: &[ParsedFile]) -> anyhow::Result<()> {
    let mut f = open("singletons.rs")?;
    define_w!(f, w, wl);
    define_xn!(xn);
    wl!("{xn}#[derive(Copy, Clone, Debug, linearize::Linearize)]");
    wl!("{xn}pub enum Singleton {{");
    {
        push_xn!(xn);
        for f in files {
            if !f.messages.singleton {
                continue;
            }
            let camel = &f.camel_obj_name;
            wl!("{xn}{camel},");
        }
    }
    wl!("{xn}}}");
    wl!();
    wl!("{xn}pub fn add_singletons(globals: &mut Globals) {{");
    {
        push_xn!(xn);
        for f in files {
            if !f.messages.singleton {
                continue;
            }
            let camel = &f.camel_obj_name;
            wl!("{xn}let name = globals.name();");
            wl!(
                "{xn}globals.add_global_no_broadcast(&Rc::new(singletons::{camel}Global::new(name)));"
            );
            wl!("{xn}globals.singletons[Singleton::{camel}] = name;");
        }
    }
    wl!("{xn}}}");
    wl!();
    wl!("{xn}#[allow(dead_code)]");
    wl!("{xn}#[allow(non_upper_case_globals)]");
    wl!("{xn}pub mod interface_singletons {{");
    {
        push_xn!(xn);
        for f in files {
            let camel = &f.camel_obj_name;
            w!("{xn}pub const {camel}: Option<crate::globals::Singleton> = ");
            if f.messages.singleton {
                wl!("Some(crate::globals::Singleton::{camel});");
            } else {
                wl!("None;");
            }
        }
    }
    wl!("{xn}}}");
    Ok(())
}

use crate::open;
use anyhow::Result;
use std::io::Write;

pub fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=src/it/tests.rs");
    let mut f = open("it_tests.rs")?;
    define_w!(f, w, wl);
    define_xn!(xn);
    let mut modules = vec![];
    for file in std::fs::read_dir("src/it/tests")? {
        let f = file?;
        let f = f.file_name();
        let Some(module) = f.to_str().unwrap().strip_suffix(".rs") else {
            continue;
        };
        modules.push(module.to_string());
    }
    modules.sort();
    wl!("{xn}pub fn tests() -> Vec<&'static dyn TestCase> {{");
    {
        push_xn!(xn);
        wl!("{xn}vec![");
        {
            push_xn!(xn);
            for m in &modules {
                wl!("{xn}&{m}::Test,");
            }
        }
        wl!("{xn}]");
    }
    wl!("{xn}}}");
    Ok(())
}

use {crate::update, anyhow::Result, kbvm::Keysym, std::fmt::Write};

pub fn main() -> Result<()> {
    let mut syms = vec![];
    for sym in Keysym::all() {
        syms.push((sym.name().unwrap(), sym));
    }
    syms.sort_by_key(|s| s.0);
    let mut res = String::new();
    writeln!(res, r#"use super::KeySym;"#)?;
    writeln!(res)?;
    for (name, sym) in syms {
        writeln!(
            res,
            r#"pub const SYM_{name}: KeySym = KeySym(0x{:x});"#,
            sym.0
        )?;
    }
    update(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../jay-config/src/keyboard/syms/generated.rs",
        ),
        &res,
    )?;
    Ok(())
}

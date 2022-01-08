use repc::layout::{Type, TypeVariant};
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::{env, io};

#[allow(unused_macros)]
#[macro_use]
#[path = "src/macros.rs"]
mod macros;

#[path = "src/pixman/consts.rs"]
mod pixman;

#[path = "src/xkbcommon/consts.rs"]
mod xkbcommon;

fn open(s: &str) -> io::Result<BufWriter<File>> {
    let mut path = PathBuf::from(env::var("OUT_DIR").unwrap());
    path.push(s);
    Ok(BufWriter::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?,
    ))
}

fn get_target() -> repc::Target {
    let rustc_target = env::var("TARGET").unwrap();
    repc::TARGET_MAP
        .iter()
        .cloned()
        .find(|t| t.0 == &rustc_target)
        .unwrap()
        .1
}

fn get_enum_ty(variants: Vec<i128>) -> anyhow::Result<u64> {
    let target = get_target();
    let ty = Type {
        layout: (),
        annotations: vec![],
        variant: TypeVariant::Enum(variants),
    };
    let ty = repc::compute_layout(target, &ty)?;
    assert!(ty.layout.pointer_alignment_bits <= ty.layout.size_bits);
    Ok(ty.layout.size_bits)
}

fn write_ty<W: Write>(f: &mut W, vals: &[u32], ty: &str) -> anyhow::Result<()> {
    let variants: Vec<_> = vals.iter().cloned().map(|v| v as i128).collect();
    let size = get_enum_ty(variants)?;
    writeln!(f, "pub type {} = u{};", ty, size)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut f = open("pixman_tys.rs")?;
    write_ty(&mut f, pixman::FORMATS, "PixmanFormat")?;
    write_ty(&mut f, pixman::OPS, "PixmanOp")?;

    let mut f = open("xkbcommon_tys.rs")?;
    write_ty(&mut f, xkbcommon::XKB_LOG_LEVEL, "xkb_log_level")?;
    write_ty(&mut f, xkbcommon::XKB_CONTEXT_FLAGS, "xkb_context_flags")?;
    write_ty(
        &mut f,
        xkbcommon::XKB_KEYMAP_COMPILE_FLAGS,
        "xkb_keymap_compile_flags",
    )?;
    write_ty(&mut f, xkbcommon::XKB_KEYMAP_FORMAT, "xkb_keymap_format")?;

    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}

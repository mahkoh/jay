use {
    crate::open,
    anyhow::{bail, Context},
    shaderc::CompileOptions,
    std::{io::Write, path::Path},
};

const ROOT: &str = "src/gfx_apis/vulkan/shaders";

pub fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed={}", ROOT);
    compile_simple("fill.frag")?;
    compile_simple("fill.vert")?;
    compile_simple("tex.vert")?;
    compile_tex_frag("tex.frag.spv", false, false)?;
    compile_tex_frag("tex.frag.mult+opaque.spv", false, true)?;
    compile_tex_frag("tex.frag.mult+alpha.spv", true, true)?;
    Ok(())
}

fn compile_tex_frag(out: &str, alpha: bool, alpha_multiplier: bool) -> anyhow::Result<()> {
    let mut opts = CompileOptions::new().unwrap();
    if alpha {
        opts.add_macro_definition("ALPHA", None);
    }
    if alpha_multiplier {
        opts.add_macro_definition("ALPHA_MULTIPLIER", None);
    }
    compile_shader("tex.frag", out, Some(&opts)).with_context(|| out.to_string())?;
    Ok(())
}

fn compile_simple(name: &str) -> anyhow::Result<()> {
    compile_shader(name, &format!("{name}.spv"), None).with_context(|| name.to_string())
}

fn compile_shader(name: &str, out: &str, options: Option<&CompileOptions>) -> anyhow::Result<()> {
    let stage = match Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "frag" => shaderc::ShaderKind::Fragment,
        "vert" => shaderc::ShaderKind::Vertex,
        n => bail!("Unknown shader stage {}", n),
    };
    let src = std::fs::read_to_string(format!("{}/{}", ROOT, name))?;
    let compiler = shaderc::Compiler::new().unwrap();
    let binary = compiler
        .compile_into_spirv(&src, stage, name, "main", options)
        .unwrap();
    let mut file = open(out)?;
    file.write_all(binary.as_binary_u8())?;
    file.flush()?;
    Ok(())
}

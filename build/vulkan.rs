use {
    crate::open,
    anyhow::{anyhow, bail, Context},
    shaderc::{CompileOptions, ResolvedInclude},
    std::{io::Write, path::Path},
};

const ROOT: &str = "src/gfx_apis/vulkan/shaders";

pub fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed={}", ROOT);
    compile_simple("fill.frag")?;
    compile_simple("fill.vert")?;
    compile_simple("tex.vert")?;
    compile_simple("tex.frag")?;
    Ok(())
}

fn compile_simple(name: &str) -> anyhow::Result<()> {
    compile_shader(name, &format!("{name}.spv"), None).with_context(|| name.to_string())
}

fn compile_shader(name: &str, out: &str, options: Option<CompileOptions>) -> anyhow::Result<()> {
    let read = |path: &str| std::fs::read_to_string(format!("{}/{}", ROOT, path));
    let mut options = options.unwrap_or_else(|| CompileOptions::new().unwrap());
    options.set_include_callback(|name, _, _, _| {
        Ok(ResolvedInclude {
            resolved_name: name.to_string(),
            content: read(name).map_err(|e| anyhow!(e).to_string())?,
        })
    });
    let stage = match Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "frag" => shaderc::ShaderKind::Fragment,
        "vert" => shaderc::ShaderKind::Vertex,
        n => bail!("Unknown shader stage {}", n),
    };
    let src = read(name)?;
    let compiler = shaderc::Compiler::new().unwrap();
    let binary = compiler
        .compile_into_spirv(&src, stage, name, "main", Some(&options))
        .unwrap();
    let mut file = open(out)?;
    file.write_all(binary.as_binary_u8())?;
    file.flush()?;
    Ok(())
}

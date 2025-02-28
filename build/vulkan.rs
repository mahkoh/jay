use {
    crate::open,
    anyhow::{Context, anyhow, bail},
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
    compile_simple("out.vert")?;
    compile_simple("out.frag")?;
    compile_simple("legacy/fill.frag")?;
    compile_simple("legacy/fill.vert")?;
    compile_simple("legacy/tex.vert")?;
    compile_simple("legacy/tex.frag")?;
    Ok(())
}

fn compile_simple(name: &str) -> anyhow::Result<()> {
    let out = format!("{name}.spv").replace("/", "_");
    compile_shader(name, &out).with_context(|| name.to_string())
}

fn compile_shader(name: &str, out: &str) -> anyhow::Result<()> {
    let root = Path::new(ROOT).join(Path::new(name).parent().unwrap());
    let read = |path: &str| std::fs::read_to_string(root.join(path));
    let mut options = CompileOptions::new().unwrap();
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
    let src = std::fs::read_to_string(format!("{}/{}", ROOT, name))?;
    let compiler = shaderc::Compiler::new().unwrap();
    let binary = compiler
        .compile_into_spirv(&src, stage, name, "main", Some(&options))
        .unwrap();
    let mut file = open(out)?;
    file.write_all(binary.as_binary_u8())?;
    file.flush()?;
    Ok(())
}

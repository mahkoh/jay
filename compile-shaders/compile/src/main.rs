use {
    anyhow::{Context, anyhow, bail},
    compile_shaders_core::{BIN, ROOT, update_hash},
    shaderc::{CompileOptions, ResolvedInclude},
    std::{fs::File, io::Write, path::Path},
};

fn main() -> anyhow::Result<()> {
    compile("fill.frag")?;
    compile("fill.vert")?;
    compile("tex.vert")?;
    compile("tex.frag")?;
    compile("out.vert")?;
    compile("out.frag")?;
    compile("legacy/fill.frag")?;
    compile("legacy/fill.vert")?;
    compile("legacy/tex.vert")?;
    compile("legacy/tex.frag")?;
    update_hash()?;
    Ok(())
}

fn compile(name: &str) -> anyhow::Result<()> {
    let out = format!("{name}.spv").replace("/", "_");
    compile_shader(name, &out).with_context(|| name.to_string())
}

fn compile_shader(name: &str, out: &str) -> anyhow::Result<()> {
    let root = Path::new(ROOT).join(Path::new(name).parent().unwrap());
    let read = |path: &str| std::fs::read_to_string(root.join(path));
    let mut options = CompileOptions::new()?;
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
    let compiler = shaderc::Compiler::new()?;
    let binary = compiler.compile_into_spirv(&src, stage, name, "main", Some(&options))?;
    let mut file = File::create(Path::new(BIN).join(out))?;
    file.write_all(binary.as_binary_u8())?;
    file.flush()?;
    Ok(())
}

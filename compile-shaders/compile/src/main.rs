use {
    anyhow::{Context, anyhow, bail},
    compile_shaders_core::{TREES, Tree, update_hash},
    shaderc::{CompileOptions, ResolvedInclude},
    std::{fs::File, io::Write, path::Path},
};

fn main() -> anyhow::Result<()> {
    for tree in TREES {
        for shader in tree.shaders {
            compile(tree, shader)?;
        }
        update_hash(tree)?;
    }
    Ok(())
}

fn compile(tree: &Tree, name: &str) -> anyhow::Result<()> {
    let out = format!("{name}.spv").replace("/", "_");
    compile_shader(tree, name, &out).with_context(|| name.to_string())
}

fn compile_shader(tree: &Tree, name: &str, out: &str) -> anyhow::Result<()> {
    let root = Path::new(tree.root).join(Path::new(name).parent().unwrap());
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
    let src = std::fs::read_to_string(format!("{}/{}", tree.root, name))?;
    let compiler = shaderc::Compiler::new()?;
    let binary = compiler.compile_into_spirv(&src, stage, name, "main", Some(&options))?;
    let mut file = File::create(Path::new(tree.bin).join(out))?;
    file.write_all(binary.as_binary_u8())?;
    file.flush()?;
    Ok(())
}

use {
    crate::open,
    anyhow::{bail, Context},
    std::{io::Write, path::Path},
};

const ROOT: &str = "src/gfx_apis/vulkan/shaders";

pub fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed={}", ROOT);
    for shader in std::fs::read_dir(ROOT)? {
        let shader = shader?;
        let name = shader.file_name().to_string_lossy().into_owned();
        compile_shader(&name).context(name)?;
    }
    Ok(())
}

fn compile_shader(name: &str) -> anyhow::Result<()> {
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
        .compile_into_spirv(&src, stage, name, "main", None)
        .unwrap();
    let mut file = open(&format!("{}.spv", name))?;
    file.write_all(binary.as_binary_u8())?;
    file.flush()?;
    Ok(())
}

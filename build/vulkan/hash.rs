use {std::fmt::Write, walkdir::WalkDir};

pub const ROOT: &str = "src/gfx_apis/vulkan/shaders";
pub const HASH: &str = "src/gfx_apis/vulkan/shaders_hash.txt";

fn calculate_hash() -> anyhow::Result<String> {
    let dir = WalkDir::new(ROOT);
    let mut files = vec![];
    for file in dir {
        let file = file?;
        if file.file_type().is_file() {
            files.push(file.path().to_path_buf());
        }
    }
    files.sort();
    let mut out = String::new();
    for file in files {
        let data = std::fs::read(&file)?;
        writeln!(out, "{} {}", blake3::hash(&data).to_hex(), file.display())?;
    }
    Ok(out)
}

pub fn unchanged() -> bool {
    let Ok(actual) = std::fs::read_to_string(HASH) else {
        return false;
    };
    let Ok(expected) = calculate_hash() else {
        return false;
    };
    actual == expected
}

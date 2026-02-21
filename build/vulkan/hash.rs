use {std::fmt::Write, walkdir::WalkDir};

#[allow(dead_code)]
pub struct Tree {
    pub root: &'static str,
    pub hash: &'static str,
    pub bin: &'static str,
    pub shaders: &'static [&'static str],
}

pub const TREES: &[Tree] = &[
    Tree {
        root: "src/gfx_apis/vulkan/shaders",
        hash: "src/gfx_apis/vulkan/shaders_hash.txt",
        bin: "src/gfx_apis/vulkan/shaders_bin",
        shaders: &[
            "fill.frag",
            "fill.vert",
            "tex.vert",
            "tex.frag",
            "out.vert",
            "out.frag",
            "legacy/fill.frag",
            "legacy/fill.vert",
            "legacy/tex.vert",
            "legacy/tex.frag",
        ],
    },
    Tree {
        root: "src/egui_adapter/shaders",
        hash: "src/egui_adapter/shaders_hash.txt",
        bin: "src/egui_adapter/shaders_bin",
        shaders: &["shader.vert", "shader.frag"],
    },
];

fn calculate_hash(tree: &Tree) -> anyhow::Result<String> {
    let dir = WalkDir::new(tree.root);
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

pub fn unchanged(tree: &Tree) -> bool {
    let Ok(actual) = std::fs::read_to_string(tree.hash) else {
        return false;
    };
    let Ok(expected) = calculate_hash(tree) else {
        return false;
    };
    actual == expected
}

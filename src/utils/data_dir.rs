use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub fn data_dir() -> &'static Path {
    static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
        let Some(mut dir) = dirs::data_local_dir() else {
            fatal!("Error: $HOME is not set");
        };
        dir.push("jay");
        dir
    });
    &DIR
}

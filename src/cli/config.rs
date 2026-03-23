use {
    crate::{
        cli::{GlobalArgs, json::jsonl},
        compositor::config_dir,
        logger::Logger,
        utils::errorfmt::ErrorFmt,
    },
    clap::{Args, Subcommand},
    jay_toml_config::CONFIG_TOML,
    std::path::Path,
    uapi::{UstrPtr, c},
};

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[clap(subcommand)]
    pub command: ConfigCmd,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Initialize the toml config file.
    Init(ConfigInitArgs),
    /// Print the path to the config.
    Path,
    /// Open the config directory with xdg-open.
    OpenDir,
}

#[derive(Args, Debug)]
pub struct ConfigInitArgs {
    /// Overwrite an existing config.toml file.
    ///
    /// The old file will be backed up to `config.toml.<n>`.
    #[clap(long)]
    overwrite: bool,
}

pub fn main(global: GlobalArgs, args: ConfigArgs) {
    Logger::install_stderr(global.log_level);
    let dir = || {
        let Some(dir) = config_dir() else {
            fatal!("Could not determine the config directory");
        };
        dir
    };
    match args.command {
        ConfigCmd::Init(a) => {
            let dir = dir();
            if let Err(e) = std::fs::create_dir_all(&dir) {
                fatal!("Could not create config directory: {}", ErrorFmt(e));
            }
            let toml_path = Path::new(&dir).join(CONFIG_TOML);
            let mut write_real_path = toml_path.clone();
            if matches!(std::fs::exists(&toml_path), Ok(true)) {
                if !a.overwrite {
                    eprintln!("{} already exists", toml_path.display());
                    eprintln!("Pass --overwrite to overwrite the config file");
                    return;
                }
                for i in 1.. {
                    write_real_path = toml_path.with_added_extension(i.to_string());
                    if matches!(std::fs::exists(&write_real_path), Ok(false)) {
                        break;
                    }
                }
            }
            let res = std::fs::write(&write_real_path, jay_toml_config::DEFAULT);
            if let Err(e) = res {
                fatal!("Could not write config: {}", ErrorFmt(e));
            }
            if write_real_path != toml_path {
                let res = uapi::renameat2(
                    c::AT_FDCWD,
                    &*toml_path,
                    c::AT_FDCWD,
                    &*write_real_path,
                    c::RENAME_EXCHANGE,
                );
                if let Err(e) = res {
                    fatal!(
                        "Could not exchange {} and {}: {}",
                        toml_path.display(),
                        write_real_path.display(),
                        ErrorFmt(e),
                    );
                }
                eprintln!("Backed up old config.toml to {}", write_real_path.display());
            }
            eprintln!("Config written to {}", toml_path.display());
        }
        ConfigCmd::Path => {
            let dir = dir();
            let toml_path = Path::new(&dir).join(CONFIG_TOML);
            if global.json {
                let path = toml_path.display().to_string();
                jsonl(&path);
            } else {
                println!("{}", toml_path.display());
            }
        }
        ConfigCmd::OpenDir => {
            const XDG_OPEN: &str = "xdg-open";
            let dir = dir();
            let mut args = UstrPtr::new();
            args.push(XDG_OPEN);
            args.push(&*dir);
            if matches!(std::fs::exists(&dir), Ok(false)) {
                fatal!("Use `jay config init` to initialize the config first");
            }
            if let Err(e) = uapi::execvp(XDG_OPEN, &args) {
                fatal!("Could not start xdg-open: {}", ErrorFmt(e));
            }
        }
    }
}

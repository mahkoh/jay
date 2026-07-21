use crate::cli::GlobalArgs;
use crate::cli::RunPrivilegedArgs;
use crate::env::WAYLAND_DISPLAY;
use crate::env::XDG_RUNTIME_DIR;
use crate::env::initial_log_level;
use crate::logger::Logger;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::oserror::OsErrorExt;
use std::path::PathBuf;
use uapi::UstrPtr;

pub fn main(_global: GlobalArgs, args: RunPrivilegedArgs) {
    Logger::install_stderr(initial_log_level());
    if let Some(xrd) = *XDG_RUNTIME_DIR {
        let mut wd = match *WAYLAND_DISPLAY {
            Some(v) => v.to_string(),
            _ => fatal!("{} is not set", WAYLAND_DISPLAY.name()),
        };
        wd.push_str(".jay");
        let mut path = PathBuf::from(xrd);
        path.push(&wd);
        if path.exists() {
            unsafe {
                std::env::set_var(WAYLAND_DISPLAY.name(), &wd);
            }
        }
    }
    let mut argv = UstrPtr::new();
    for arg in &args.program {
        argv.push(arg.as_str());
    }
    let program = args.program[0].as_str();
    let res = uapi::execvp(program, &argv).to_os_error().unwrap_err();
    fatal!("Could not execute `{}`: {}", program, ErrorFmt(res));
}

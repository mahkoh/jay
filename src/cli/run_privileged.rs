use {
    crate::{
        cli::{GlobalArgs, RunPrivilegedArgs},
        compositor::WAYLAND_DISPLAY,
        logger::Logger,
        utils::{errorfmt::ErrorFmt, oserror::OsError},
    },
    uapi::UstrPtr,
};

pub fn main(global: GlobalArgs, args: RunPrivilegedArgs) {
    Logger::install_stderr(global.log_level.into());
    let mut wd = match std::env::var(WAYLAND_DISPLAY) {
        Ok(v) => v,
        _ => fatal!("{} is not set", WAYLAND_DISPLAY),
    };
    wd.push_str(".jay");
    std::env::set_var(WAYLAND_DISPLAY, &wd);
    let mut argv = UstrPtr::new();
    for arg in &args.program {
        argv.push(arg.as_str());
    }
    let program = args.program[0].as_str();
    let res = uapi::execvp(program, &argv).unwrap_err();
    fatal!(
        "Could not execute `{}`: {}",
        program,
        ErrorFmt(OsError::from(res))
    );
}

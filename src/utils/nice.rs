use {
    crate::{compositor::config_dir, config::have_config_so},
    c::sched_setscheduler,
    std::{
        env, mem,
        sync::atomic::{AtomicBool, Ordering::Relaxed},
    },
    uapi::c::{self, SCHED_RESET_ON_FORK, SCHED_RR, sched_param},
};

static DID_ELEVATE_SCHEDULER: AtomicBool = AtomicBool::new(false);

pub const JAY_NO_REALTIME: &str = "JAY_NO_REALTIME";

pub fn elevate_scheduler() {
    if env::var(JAY_NO_REALTIME).as_deref().unwrap_or_default() == "1" {
        return;
    }
    if have_config_so(config_dir().as_deref()) && dont_allow_realtime_config_so() {
        return;
    }
    let mut param = unsafe { mem::zeroed::<sched_param>() };
    param.sched_priority = 1;
    let res = unsafe { sched_setscheduler(0, SCHED_RR | SCHED_RESET_ON_FORK, &param) };
    if res == 0 {
        DID_ELEVATE_SCHEDULER.store(true, Relaxed);
    }
}

pub fn did_elevate_scheduler() -> bool {
    DID_ELEVATE_SCHEDULER.load(Relaxed)
}

fn dont_allow_realtime_config_so() -> bool {
    option_env!(jay_allow_realtime_config_so!()).unwrap_or_default() != "1"
}

pub fn dont_allow_config_so() -> bool {
    did_elevate_scheduler() && dont_allow_realtime_config_so()
}

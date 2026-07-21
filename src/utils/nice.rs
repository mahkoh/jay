use crate::config::is_unprivileged_config_so;
use crate::config::open_config_so;
use crate::env::JAY_NO_REALTIME;
use crate::env::config_dir;
use c::sched_setscheduler;
use std::mem;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use uapi::c::SCHED_RESET_ON_FORK;
use uapi::c::SCHED_RR;
use uapi::c::sched_param;
use uapi::c::{self};

static DID_ELEVATE_SCHEDULER: AtomicBool = AtomicBool::new(false);

pub fn elevate_scheduler() {
    if *JAY_NO_REALTIME {
        return;
    }
    if dont_allow_realtime_config_so()
        && let Ok(fd) = open_config_so(config_dir())
        && let Ok(stat) = uapi::fstat(fd.raw())
        && is_unprivileged_config_so(&stat)
    {
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

pub fn dont_allow_unprivileged_config_so() -> bool {
    did_elevate_scheduler() && dont_allow_realtime_config_so()
}

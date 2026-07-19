use crate::utils::clone3::double_fork;
use crate::utils::clone3::set_deathsig;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fd_blocker::FdBlocker;
use crate::utils::fd_blocker::create_fd_blocker;
use crate::utils::process_name::set_process_name;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

pub struct Sleeper {
    _blocker: FdBlocker,
    pub pidfd: Rc<OwnedFd>,
}

pub fn start_sleeper() -> Sleeper {
    let (blocker, barrier) = match create_fd_blocker() {
        Ok(r) => r,
        Err(e) => panic!("Could not create fd blocker: {}", ErrorFmt(e)),
    };
    let res = match double_fork() {
        Ok(r) => r,
        Err(e) => panic!("Could not fork sleeper: {}", ErrorFmt(e)),
    };
    match res {
        Some(pidfd) => Sleeper {
            _blocker: blocker,
            pidfd: Rc::new(pidfd),
        },
        _ => {
            set_process_name("jay sleeper");
            set_deathsig();
            drop(blocker);
            let _ = barrier.wait_blocking();
            unsafe {
                c::_exit(0);
            }
        }
    }
}

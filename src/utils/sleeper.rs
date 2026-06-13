use {
    crate::utils::{
        clone3::{double_fork, set_deathsig},
        errorfmt::ErrorFmt,
        fd_blocker::{FdBlocker, create_fd_blocker},
        process_name::set_process_name,
    },
    std::rc::Rc,
    uapi::{OwnedFd, c},
};

pub struct Sleeper {
    _blocker: FdBlocker,
    #[expect(dead_code)]
    pub pidfd: Rc<OwnedFd>,
}

#[expect(dead_code)]
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

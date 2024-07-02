use {
    crate::forker::ForkerError,
    std::mem,
    uapi::{c, OwnedFd},
};

#[derive(Default, Copy, Clone)]
#[allow(non_camel_case_types, dead_code)]
struct clone_args {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

pub enum Forked {
    Parent { pid: c::pid_t, pidfd: OwnedFd },
    Child { _pidfd: Option<OwnedFd> },
}

pub fn fork_with_pidfd(pidfd_for_child: bool) -> Result<Forked, ForkerError> {
    let mut pidfd: c::c_int = 0;
    let mut args = clone_args {
        flags: c::CLONE_PIDFD as u64,
        pidfd: (&mut pidfd as *mut c::c_int) as _,
        exit_signal: c::SIGCHLD as _,
        ..Default::default()
    };
    let mut child_pidfd = None;
    if pidfd_for_child {
        child_pidfd = Some(uapi::pidfd_open(uapi::getpid(), 0).unwrap());
    }
    unsafe {
        let pid = c::syscall(
            c::SYS_clone3,
            &mut args as *const _ as usize,
            mem::size_of::<clone_args>(),
        );
        if let Err(e) = uapi::map_err!(pid) {
            return Err(ForkerError::Fork(e.into()));
        }
        let res = if pid == 0 {
            Forked::Child {
                _pidfd: child_pidfd,
            }
        } else {
            Forked::Parent {
                pid: pid as _,
                pidfd: OwnedFd::new(pidfd),
            }
        };
        Ok(res)
    }
}

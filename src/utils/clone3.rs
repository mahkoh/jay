use {
    crate::{
        forker::ForkerError,
        utils::{errorfmt::ErrorFmt, on_drop::OnDrop, process_name::set_process_name},
    },
    std::{env, mem::MaybeUninit, process, slice, str::FromStr},
    uapi::{Msghdr, MsghdrMut, OwnedFd, c},
};

#[derive(Default, Copy, Clone)]
#[expect(non_camel_case_types, dead_code)]
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
    Child { pidfd: Option<OwnedFd> },
}

const REAPER_VAR: &str = "JAY_REAPER_PID";

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
            size_of::<clone_args>(),
        );
        if let Err(e) = uapi::map_err!(pid) {
            return Err(ForkerError::Fork(e.into()));
        }
        let res = if pid == 0 {
            env::remove_var(REAPER_VAR);
            Forked::Child { pidfd: child_pidfd }
        } else {
            Forked::Parent {
                pid: pid as _,
                pidfd: OwnedFd::new(pidfd),
            }
        };
        Ok(res)
    }
}

pub fn double_fork() -> Result<Option<OwnedFd>, ForkerError> {
    let (p, c) = uapi::socketpair(c::AF_UNIX, c::SOCK_DGRAM | c::SOCK_CLOEXEC, 0)
        .map_err(|e| ForkerError::Socketpair(e.into()))?;
    match fork_with_pidfd(false)? {
        Forked::Parent { pid, .. } => {
            drop(c);
            let mut buf = [MaybeUninit::<u8>::uninit(); 128];
            let iov: &mut [&mut [u8]] = &mut [];
            let mut msghdr = MsghdrMut {
                iov,
                control: Some(&mut buf),
                name: uapi::sockaddr_none_mut(),
                flags: 0,
            };
            let _wait = OnDrop(|| {
                let _ = uapi::waitpid(pid, 0);
            });
            let (_, _, mut ctrl) = uapi::recvmsg(p.raw(), &mut msghdr, c::MSG_CMSG_CLOEXEC)
                .map_err(|e| ForkerError::RecvPidfd(e.into()))?;
            let (_, hdr, data) =
                uapi::cmsg_read(&mut ctrl).map_err(|e| ForkerError::CmsgRead(e.into()))?;
            if hdr.cmsg_level != c::SOL_SOCKET || hdr.cmsg_type != c::SCM_RIGHTS {
                return Err(ForkerError::InvalidCmsg);
            }
            let Ok(fd) = uapi::pod_read(data) else {
                return Err(ForkerError::InvalidCmsg);
            };
            Ok(Some(OwnedFd::new(fd)))
        }
        Forked::Child { .. } => {
            drop(p);
            if let Ok(f) = fork_with_pidfd(true) {
                match f {
                    Forked::Parent { pidfd, .. } => {
                        let pidfd = pidfd.raw();
                        let mut buf = [MaybeUninit::uninit(); 128];
                        let mut hdr: c::cmsghdr = uapi::pod_zeroed();
                        hdr.cmsg_level = c::SOL_SOCKET;
                        hdr.cmsg_type = c::SCM_RIGHTS;
                        let _ = uapi::cmsg_write(&mut &mut buf[..], hdr, &pidfd);
                        let iov: &[&[u8]] = &[];
                        let msghdr = Msghdr {
                            iov,
                            control: Some(&buf[..uapi::cmsg_space(size_of_val(&pidfd))]),
                            name: uapi::sockaddr_none_ref(),
                        };
                        let _ = uapi::sendmsg(c.raw(), &msghdr, 0);
                    }
                    Forked::Child { pidfd } => {
                        let pidfd = pidfd.unwrap();
                        let mut pollfd = c::pollfd {
                            fd: pidfd.raw(),
                            events: c::POLLIN as _,
                            revents: 0,
                        };
                        let _ = uapi::poll(slice::from_mut(&mut pollfd), -1);
                        return Ok(None);
                    }
                }
            };
            unsafe {
                c::_exit(0);
            }
        }
    }
}

pub fn ensure_reaper() -> c::pid_t {
    if let Ok(id) = env::var(REAPER_VAR) {
        if let Ok(id) = c::pid_t::from_str(&id) {
            if uapi::getppid() == id {
                set_deathsig();
                return id;
            }
        }
    }
    let reaper_pid = uapi::getpid();
    unsafe {
        c::prctl(c::PR_SET_CHILD_SUBREAPER, 1);
    }
    let res = match fork_with_pidfd(false) {
        Ok(r) => r,
        Err(e) => {
            fatal!("Could not fork reaper: {}", ErrorFmt(e));
        }
    };
    let Forked::Parent {
        pid: main_process_id,
        ..
    } = res
    else {
        unsafe {
            env::set_var(REAPER_VAR, reaper_pid.to_string());
        }
        set_deathsig();
        return reaper_pid;
    };
    set_process_name("jay reaper");
    while let Ok((pid, status)) = uapi::wait() {
        if pid == main_process_id {
            process::exit(uapi::WEXITSTATUS(status));
        }
    }
    process::exit(1);
}

fn set_deathsig() {
    unsafe {
        c::prctl(c::PR_SET_PDEATHSIG, c::SIGKILL as c::c_ulong);
    }
}

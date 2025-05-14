use {
    crate::utils::{errorfmt::ErrorFmt, oserror::OsError},
    bstr::ByteSlice,
    std::os::unix::ffi::OsStrExt,
    uapi::{OwnedFd, c},
};

pub struct PidInfo {
    pub uid: c::uid_t,
    pub pid: c::pid_t,
    pub comm: String,
    pub exe: String,
}

pub fn get_pid_info(uid: c::uid_t, pid: c::pid_t) -> PidInfo {
    let comm = match std::fs::read(format!("/proc/{}/comm", pid)) {
        Ok(name) => name.trim_ascii_end().as_bstr().to_string(),
        Err(e) => {
            log::warn!("Could not read `comm` of pid {}: {}", pid, ErrorFmt(e));
            "Unknown".to_string()
        }
    };
    let exe = match std::fs::read_link(format!("/proc/{}/exe", pid)) {
        Ok(name) => name
            .as_os_str()
            .as_bytes()
            .trim_ascii_end()
            .as_bstr()
            .to_string(),
        Err(e) => {
            log::warn!("Could not read `exe` of pid {}: {}", pid, ErrorFmt(e));
            "Unknown".to_string()
        }
    };
    PidInfo {
        uid,
        pid,
        comm,
        exe,
    }
}

pub fn get_socket_creds(socket: &OwnedFd) -> Option<(c::uid_t, c::pid_t)> {
    let mut cred = c::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    match uapi::getsockopt(socket.raw(), c::SOL_SOCKET, c::SO_PEERCRED, &mut cred) {
        Ok(_) => Some((cred.uid, cred.pid)),
        Err(e) => {
            log::error!(
                "Cannot determine peer credentials of new connection: {:?}",
                OsError::from(e)
            );
            None
        }
    }
}

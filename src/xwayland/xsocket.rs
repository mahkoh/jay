use {
    crate::{utils::errorfmt::ErrorFmt, xwayland::XWaylandError},
    std::{
        io::{Read, Write},
        rc::Rc,
    },
    uapi::{Errno, OwnedFd, Ustring, c, format_ustr},
};

const SOCK_DIR: &str = "/tmp/.X11-unix";

pub struct XSocket {
    pub id: u32,
    pub path: Ustring,
    pub lock_path: Ustring,
}

impl Drop for XSocket {
    fn drop(&mut self) {
        let _ = uapi::unlink(&self.path);
        let _ = uapi::unlink(&self.lock_path);
    }
}

fn bind_socket(fd: &Rc<OwnedFd>, id: u32) -> Result<(XSocket, Rc<OwnedFd>), XWaylandError> {
    let path = format_ustr!("{}/X{}", SOCK_DIR, id);
    let lock_path = format_ustr!("/tmp/.X{}-lock", id);
    let mut lock_fd = 'open_lock_file: {
        for i in 0..2 {
            if let Ok(fd) = uapi::open(
                &*lock_path,
                c::O_CREAT | c::O_CLOEXEC | c::O_WRONLY | c::O_EXCL,
                0o444,
            ) {
                break 'open_lock_file fd;
            }
            if i == 1 {
                return Err(XWaylandError::AlreadyInUse);
            }
            let mut fd = match uapi::open(&*lock_path, c::O_CLOEXEC | c::O_RDONLY, 0) {
                Ok(f) => f,
                Err(e) => return Err(XWaylandError::ReadLockFile(e.into())),
            };
            let mut pid = String::new();
            if let Err(e) = fd.read_to_string(&mut pid) {
                return Err(XWaylandError::ReadLockFile(e.into()));
            }
            let pid = match pid.trim().parse() {
                Ok(p) => p,
                Err(e) => return Err(XWaylandError::NotALockFile(e)),
            };
            match uapi::kill(pid, 0) {
                Err(Errno(c::ESRCH)) => {
                    let _ = uapi::unlink(&lock_path);
                }
                _ => return Err(XWaylandError::AlreadyInUse),
            }
        }
        return Err(XWaylandError::AlreadyInUse);
    };
    let _ = uapi::unlink(&path);
    let mut addr: c::sockaddr_un = uapi::pod_zeroed();
    addr.sun_family = c::AF_UNIX as _;
    let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
    sun_path[..path.len()].copy_from_slice(path.as_bytes());
    sun_path[path.len()] = 0;
    if let Err(e) = uapi::bind(fd.raw(), &addr) {
        return Err(XWaylandError::BindFailed(e.into()));
    }
    let s = format!("{:10}\n", uapi::getpid());
    if let Err(e) = lock_fd.write_all(s.as_bytes()) {
        return Err(XWaylandError::WriteLockFile(e.into()));
    }
    let xsocket = XSocket {
        id,
        path,
        lock_path,
    };
    Ok((xsocket, fd.clone()))
}

pub(super) fn allocate_socket() -> Result<(XSocket, Rc<OwnedFd>), XWaylandError> {
    match uapi::stat(SOCK_DIR) {
        Err(Errno(c::ENOENT)) => return Err(XWaylandError::MissingSocketDir),
        Err(e) => return Err(XWaylandError::StatSocketDir(e.into())),
        Ok(s) if s.st_mode & c::S_IFMT != c::S_IFDIR => return Err(XWaylandError::NotASocketDir),
        _ => {
            if uapi::access(SOCK_DIR, c::W_OK).is_err() {
                return Err(XWaylandError::SocketDirNotWritable);
            }
        }
    }
    let fd = match uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0) {
        Ok(f) => Rc::new(f),
        Err(e) => return Err(XWaylandError::SocketFailed(e.into())),
    };
    for i in 500..1500 {
        match bind_socket(&fd, i) {
            Ok(s) => return Ok(s),
            Err(e) => {
                log::warn!("Cannot use the :{} display: {}", i, ErrorFmt(e));
            }
        }
    }
    Err(XWaylandError::AddressesInUse)
}

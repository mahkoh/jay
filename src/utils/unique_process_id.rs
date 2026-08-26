use crate::time::Time;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::ioctl::ioctl;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt2;
use std::sync::LazyLock;
use std::thread;
use thiserror::Error;
use uapi::c;
use uapi::gettid;

#[derive(Debug, Error)]
enum E {
    #[error("pidfd_open failed")]
    PidfdOpen(#[source] OsError),
    #[error("fstatfs failed")]
    Fstatfs(#[source] OsError),
    #[error("pidfs is not available")]
    NotPidFs,
    #[error("fstat failed")]
    Fstat(#[source] OsError),
    #[error("FS_IOC_GETVERSION failed")]
    Getversion(#[source] OsError),
}

#[expect(unused)]
pub fn unique_process_id() -> u64 {
    static ONCE: LazyLock<u64> = LazyLock::new(compute);
    *ONCE
}

fn compute() -> u64 {
    if let Some(v) = pidfd() {
        return v;
    }
    tid()
}

fn pidfd() -> Option<u64> {
    thread::spawn(|| {
        try_pidfd()
            .inspect_err(|e| {
                log::warn!("pidfd method failed: {}", ErrorFmt(e));
            })
            .ok()
    })
    .join()
    .unwrap()
}

fn try_pidfd() -> Result<u64, E> {
    const PID_FS_MAGIC: u64 = 0x50494446;
    let fd = uapi::pidfd_open(gettid(), c::PIDFD_THREAD).map_os_err(E::PidfdOpen)?;
    let statfs = uapi::fstatfs(fd.raw()).map_os_err(E::Fstatfs)?;
    if statfs.f_type as u64 != PID_FS_MAGIC {
        return Err(E::NotPidFs);
    }
    let stat = uapi::fstat(fd.raw()).map_os_err(E::Fstat)?;
    let lo = stat.st_ino as u64;
    let hi = fs_ioc_getversion(fd.raw()).map_err(E::Getversion)?;
    let res = (hi << 32) | lo;
    Ok(res)
}

fn fs_ioc_getversion(fd: c::c_int) -> Result<u64, OsError> {
    const FS_IOC_GETVERSION: u64 = uapi::_IOR::<c::c_long>(b'v' as u64, 1);
    let mut res: c::c_long = 0;
    unsafe {
        ioctl(fd, FS_IOC_GETVERSION, &mut res)?;
    }
    Ok(res as u64)
}

fn tid() -> u64 {
    let lo = Time::now_unchecked().msec() / 100;
    let hi = thread::spawn(gettid).join().unwrap() as u64;
    (hi << 32) | (lo as u32 as u64)
}

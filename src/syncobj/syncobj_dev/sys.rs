use {
    crate::utils::{ioctl::ioctl, oserror::OsError},
    uapi::{OwnedFd, c},
};

pub const SYNCOBJ_CREATE_SIGNALED: u32 = 1 << 0;

#[expect(dead_code)]
pub const SYNCOBJ_WAIT_FLAGS_WAIT_ALL: u32 = 1 << 0;
pub const SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT: u32 = 1 << 1;
pub const SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE: u32 = 1 << 2;
#[expect(dead_code)]
pub const SYNCOBJ_WAIT_FLAGS_WAIT_DEADLINE: u32 = 1 << 3;

#[expect(dead_code)]
pub const SYNCOBJ_QUERY_FLAGS_LAST_SUBMITTED: u32 = 1 << 0;

#[repr(C)]
struct syncobj_create_args {
    fd: i32,
    flags: u32,
}

#[repr(C)]
struct syncobj_wait_args {
    fds: u64,
    points: u64,
    timeout_nsec: i64,
    count: u32,
    flags: u32,
    first_signaled: u32,
    pad: u32,
    deadline_nsec: u64,
}

#[repr(C)]
struct syncobj_array_args {
    fds: u64,
    points: u64,
    count: u32,
    flags: u32,
}

#[repr(C)]
struct syncobj_transfer_args {
    src_fd: i32,
    dst_fd: i32,
    src_point: u64,
    dst_point: u64,
    flags: u32,
    pad: u32,
}

#[repr(C)]
struct syncobj_eventfd_args {
    syncobj_fd: i32,
    eventfd: i32,
    point: u64,
    flags: u32,
    pad: u32,
}

#[repr(C)]
struct syncobj_sync_file_args {
    syncobj_fd: i32,
    sync_file_fd: i32,
    point: u64,
}

const SYNCOBJ_IOC_BASE: u64 = 0xCD;

const SYNCOBJ_IOC_CREATE: u64 = uapi::_IOWR::<syncobj_create_args>(SYNCOBJ_IOC_BASE, 0);
const SYNCOBJ_IOC_WAIT: u64 = uapi::_IOWR::<syncobj_wait_args>(SYNCOBJ_IOC_BASE, 1);
#[expect(dead_code)]
const SYNCOBJ_IOC_RESET: u64 = uapi::_IOW::<syncobj_array_args>(SYNCOBJ_IOC_BASE, 2);
const SYNCOBJ_IOC_SIGNAL: u64 = uapi::_IOW::<syncobj_array_args>(SYNCOBJ_IOC_BASE, 3);
const SYNCOBJ_IOC_QUERY: u64 = uapi::_IOW::<syncobj_array_args>(SYNCOBJ_IOC_BASE, 4);
const SYNCOBJ_IOC_TRANSFER: u64 = uapi::_IOW::<syncobj_transfer_args>(SYNCOBJ_IOC_BASE, 5);
const SYNCOBJ_IOC_EVENTFD: u64 = uapi::_IOW::<syncobj_eventfd_args>(SYNCOBJ_IOC_BASE, 6);
const SYNCOBJ_IOC_EXPORT_SYNC_FILE: u64 =
    uapi::_IOWR::<syncobj_sync_file_args>(SYNCOBJ_IOC_BASE, 7);
const SYNCOBJ_IOC_IMPORT_SYNC_FILE: u64 = uapi::_IOW::<syncobj_sync_file_args>(SYNCOBJ_IOC_BASE, 8);

pub fn syncobj_create(dev: c::c_int, flags: u32) -> Result<OwnedFd, OsError> {
    let mut res = syncobj_create_args { fd: 0, flags };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_CREATE, &mut res)?;
    }
    Ok(OwnedFd::new(res.fd))
}

pub fn syncobj_export_sync_file(
    dev: c::c_int,
    syncobj_fd: c::c_int,
    point: u64,
) -> Result<OwnedFd, OsError> {
    let mut res = syncobj_sync_file_args {
        syncobj_fd: syncobj_fd as i32,
        sync_file_fd: 0,
        point,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_EXPORT_SYNC_FILE, &mut res)?;
    }
    Ok(OwnedFd::new(res.sync_file_fd))
}

pub fn syncobj_import_sync_file(
    dev: c::c_int,
    syncobj_fd: c::c_int,
    point: u64,
    sync_file_fd: c::c_int,
) -> Result<(), OsError> {
    let mut res = syncobj_sync_file_args {
        syncobj_fd: syncobj_fd as i32,
        sync_file_fd: sync_file_fd as i32,
        point,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_IMPORT_SYNC_FILE, &mut res)?;
    }
    Ok(())
}

pub fn syncobj_eventfd(
    dev: c::c_int,
    eventfd: c::c_int,
    syncobj_fd: c::c_int,
    point: u64,
    flags: u32,
) -> Result<(), OsError> {
    let mut res = syncobj_eventfd_args {
        syncobj_fd: syncobj_fd as i32,
        eventfd: eventfd as i32,
        point,
        flags,
        pad: 0,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_EVENTFD, &mut res)?;
    }
    Ok(())
}

pub fn syncobj_signal(dev: c::c_int, fd: c::c_int, point: u64) -> Result<(), OsError> {
    let fd = fd as i32;
    let mut res = syncobj_array_args {
        fds: &fd as *const i32 as u64,
        points: &point as *const u64 as u64,
        count: 1,
        flags: 0,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_SIGNAL, &mut res)?;
    }
    Ok(())
}

pub fn syncobj_query(dev: c::c_int, fd: c::c_int) -> Result<u64, OsError> {
    let fd = fd as i32;
    let mut point = 0u64;
    let mut res = syncobj_array_args {
        fds: &fd as *const i32 as u64,
        points: &mut point as *mut u64 as u64,
        count: 1,
        flags: 0,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_QUERY, &mut res)?;
    }
    Ok(point)
}

#[expect(dead_code)]
pub fn syncobj_transfer(
    dev: c::c_int,
    src_fd: c::c_int,
    src_point: u64,
    dst_fd: c::c_int,
    dst_point: u64,
    flags: u32,
) -> Result<(), OsError> {
    let mut res = syncobj_transfer_args {
        src_fd: src_fd as i32,
        dst_fd: dst_fd as i32,
        src_point,
        dst_point,
        flags,
        pad: 0,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_TRANSFER, &mut res)?;
    }
    Ok(())
}

pub fn syncobj_wait(
    dev: c::c_int,
    syncobj: c::c_int,
    point: u64,
    flags: u32,
) -> Result<(), OsError> {
    let fd = syncobj as i32;
    let mut res = syncobj_wait_args {
        fds: &fd as *const i32 as u64,
        points: &point as *const u64 as u64,
        timeout_nsec: i64::MAX,
        count: 1,
        flags,
        first_signaled: 0,
        pad: 0,
        deadline_nsec: 0,
    };
    unsafe {
        ioctl(dev, SYNCOBJ_IOC_WAIT, &mut res)?;
    }
    Ok(())
}

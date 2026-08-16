#![allow(unused, non_snake_case)]

use jay_proc::Pod;
use std::mem::offset_of;
use uapi::Packed;

pub const FUSE_KERNEL_VERSION: u32 = 7;
pub const FUSE_KERNEL_MINOR_VERSION: u32 = 45;
pub const FUSE_ROOT_ID: u64 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_attr {
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub atimensec: u32,
    pub mtimensec: u32,
    pub ctimensec: u32,
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u32,
    pub blksize: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_sx_time {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __reserved: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_statx {
    pub mask: u32,
    pub blksize: u32,
    pub attributes: u64,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub mode: u16,
    pub __spare0: [u16; 1],
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub attributes_mask: u64,
    pub atime: fuse_sx_time,
    pub btime: fuse_sx_time,
    pub ctime: fuse_sx_time,
    pub mtime: fuse_sx_time,
    pub rdev_major: u32,
    pub rdev_minor: u32,
    pub dev_major: u32,
    pub dev_minor: u32,
    pub __spare2: [u64; 14],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_kstatfs {
    pub blocks: u64,
    pub bfree: u64,
    pub bavail: u64,
    pub files: u64,
    pub ffree: u64,
    pub bsize: u32,
    pub namelen: u32,
    pub frsize: u32,
    pub padding: u32,
    pub spare: [u32; 6],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_file_lock {
    pub start: u64,
    pub end: u64,
    pub type_: u32,
    pub pid: u32,
}

pub const FATTR_MODE: u32 = 1 << 0;
pub const FATTR_UID: u32 = 1 << 1;
pub const FATTR_GID: u32 = 1 << 2;
pub const FATTR_SIZE: u32 = 1 << 3;
pub const FATTR_ATIME: u32 = 1 << 4;
pub const FATTR_MTIME: u32 = 1 << 5;
pub const FATTR_FH: u32 = 1 << 6;
pub const FATTR_ATIME_NOW: u32 = 1 << 7;
pub const FATTR_MTIME_NOW: u32 = 1 << 8;
pub const FATTR_LOCKOWNER: u32 = 1 << 9;
pub const FATTR_CTIME: u32 = 1 << 10;
pub const FATTR_KILL_SUIDGID: u32 = 1 << 11;

pub const FOPEN_DIRECT_IO: u32 = 1 << 0;
pub const FOPEN_KEEP_CACHE: u32 = 1 << 1;
pub const FOPEN_NONSEEKABLE: u32 = 1 << 2;
pub const FOPEN_CACHE_DIR: u32 = 1 << 3;
pub const FOPEN_STREAM: u32 = 1 << 4;
pub const FOPEN_NOFLUSH: u32 = 1 << 5;
pub const FOPEN_PARALLEL_DIRECT_WRITES: u32 = 1 << 6;
pub const FOPEN_PASSTHROUGH: u32 = 1 << 7;

pub const FUSE_ASYNC_READ: u64 = 1 << 0;
pub const FUSE_POSIX_LOCKS: u64 = 1 << 1;
pub const FUSE_FILE_OPS: u64 = 1 << 2;
pub const FUSE_ATOMIC_O_TRUNC: u64 = 1 << 3;
pub const FUSE_EXPORT_SUPPORT: u64 = 1 << 4;
pub const FUSE_BIG_WRITES: u64 = 1 << 5;
pub const FUSE_DONT_MASK: u64 = 1 << 6;
pub const FUSE_SPLICE_WRITE: u64 = 1 << 7;
pub const FUSE_SPLICE_MOVE: u64 = 1 << 8;
pub const FUSE_SPLICE_READ: u64 = 1 << 9;
pub const FUSE_FLOCK_LOCKS: u64 = 1 << 10;
pub const FUSE_HAS_IOCTL_DIR: u64 = 1 << 11;
pub const FUSE_AUTO_INVAL_DATA: u64 = 1 << 12;
pub const FUSE_DO_READDIRPLUS: u64 = 1 << 13;
pub const FUSE_READDIRPLUS_AUTO: u64 = 1 << 14;
pub const FUSE_ASYNC_DIO: u64 = 1 << 15;
pub const FUSE_WRITEBACK_CACHE: u64 = 1 << 16;
pub const FUSE_NO_OPEN_SUPPORT: u64 = 1 << 17;
pub const FUSE_PARALLEL_DIROPS: u64 = 1 << 18;
pub const FUSE_HANDLE_KILLPRIV: u64 = 1 << 19;
pub const FUSE_POSIX_ACL: u64 = 1 << 20;
pub const FUSE_ABORT_ERROR: u64 = 1 << 21;
pub const FUSE_MAX_PAGES: u64 = 1 << 22;
pub const FUSE_CACHE_SYMLINKS: u64 = 1 << 23;
pub const FUSE_NO_OPENDIR_SUPPORT: u64 = 1 << 24;
pub const FUSE_EXPLICIT_INVAL_DATA: u64 = 1 << 25;
pub const FUSE_MAP_ALIGNMENT: u64 = 1 << 26;
pub const FUSE_SUBMOUNTS: u64 = 1 << 27;
pub const FUSE_HANDLE_KILLPRIV_V2: u64 = 1 << 28;
pub const FUSE_SETXATTR_EXT: u64 = 1 << 29;
pub const FUSE_INIT_EXT: u64 = 1 << 30;
pub const FUSE_INIT_RESERVED: u64 = 1 << 31;
pub const FUSE_SECURITY_CTX: u64 = 1 << 32;
pub const FUSE_HAS_INODE_DAX: u64 = 1 << 33;
pub const FUSE_CREATE_SUPP_GROUP: u64 = 1 << 34;
pub const FUSE_HAS_EXPIRE_ONLY: u64 = 1 << 35;
pub const FUSE_DIRECT_IO_ALLOW_MMAP: u64 = 1 << 36;
pub const FUSE_PASSTHROUGH: u64 = 1 << 37;
pub const FUSE_NO_EXPORT_SUPPORT: u64 = 1 << 38;
pub const FUSE_HAS_RESEND: u64 = 1 << 39;
pub const FUSE_DIRECT_IO_RELAX: u64 = FUSE_DIRECT_IO_ALLOW_MMAP;
pub const FUSE_ALLOW_IDMAP: u64 = 1 << 40;
pub const FUSE_OVER_IO_URING: u64 = 1 << 41;
pub const FUSE_REQUEST_TIMEOUT: u64 = 1 << 42;

pub const CUSE_UNRESTRICTED_IOCTL: u32 = 1 << 0;

pub const FUSE_RELEASE_FLUSH: u32 = 1 << 0;
pub const FUSE_RELEASE_FLOCK_UNLOCK: u32 = 1 << 1;

pub const FUSE_GETATTR_FH: u32 = 1 << 0;

pub const FUSE_LK_FLOCK: u32 = 1 << 0;

pub const FUSE_WRITE_CACHE: u32 = 1 << 0;
pub const FUSE_WRITE_LOCKOWNER: u32 = 1 << 1;
pub const FUSE_WRITE_KILL_SUIDGID: u32 = 1 << 2;
pub const FUSE_WRITE_KILL_PRIV: u32 = FUSE_WRITE_KILL_SUIDGID;

pub const FUSE_READ_LOCKOWNER: u32 = 1 << 1;

pub const FUSE_IOCTL_COMPAT: u32 = 1 << 0;
pub const FUSE_IOCTL_UNRESTRICTED: u32 = 1 << 1;
pub const FUSE_IOCTL_RETRY: u32 = 1 << 2;
pub const FUSE_IOCTL_32BIT: u32 = 1 << 3;
pub const FUSE_IOCTL_DIR: u32 = 1 << 4;
pub const FUSE_IOCTL_COMPAT_X32: u32 = 1 << 5;
pub const FUSE_IOCTL_MAX_IOV: u32 = 256;

pub const FUSE_POLL_SCHEDULE_NOTIFY: u32 = 1 << 0;

pub const FUSE_FSYNC_FDATASYNC: u32 = 1 << 0;

pub const FUSE_ATTR_SUBMOUNT: u32 = 1 << 0;
pub const FUSE_ATTR_DAX: u32 = 1 << 1;

pub const FUSE_OPEN_KILL_SUIDGID: u32 = 1 << 0;

pub const FUSE_SETXATTR_ACL_KILL_SGID: u32 = 1 << 0;

pub const FUSE_EXPIRE_ONLY: u32 = 1 << 0;

pub const FUSE_MAX_NR_SECCTX: u32 = 31;
pub const FUSE_EXT_GROUPS: u32 = 32;

pub const FUSE_LOOKUP: u32 = 1;
pub const FUSE_FORGET: u32 = 2;
pub const FUSE_GETATTR: u32 = 3;
pub const FUSE_SETATTR: u32 = 4;
pub const FUSE_READLINK: u32 = 5;
pub const FUSE_SYMLINK: u32 = 6;
pub const FUSE_MKNOD: u32 = 8;
pub const FUSE_MKDIR: u32 = 9;
pub const FUSE_UNLINK: u32 = 10;
pub const FUSE_RMDIR: u32 = 11;
pub const FUSE_RENAME: u32 = 12;
pub const FUSE_LINK: u32 = 13;
pub const FUSE_OPEN: u32 = 14;
pub const FUSE_READ: u32 = 15;
pub const FUSE_WRITE: u32 = 16;
pub const FUSE_STATFS: u32 = 17;
pub const FUSE_RELEASE: u32 = 18;
pub const FUSE_FSYNC: u32 = 20;
pub const FUSE_SETXATTR: u32 = 21;
pub const FUSE_GETXATTR: u32 = 22;
pub const FUSE_LISTXATTR: u32 = 23;
pub const FUSE_REMOVEXATTR: u32 = 24;
pub const FUSE_FLUSH: u32 = 25;
pub const FUSE_INIT: u32 = 26;
pub const FUSE_OPENDIR: u32 = 27;
pub const FUSE_READDIR: u32 = 28;
pub const FUSE_RELEASEDIR: u32 = 29;
pub const FUSE_FSYNCDIR: u32 = 30;
pub const FUSE_GETLK: u32 = 31;
pub const FUSE_SETLK: u32 = 32;
pub const FUSE_SETLKW: u32 = 33;
pub const FUSE_ACCESS: u32 = 34;
pub const FUSE_CREATE: u32 = 35;
pub const FUSE_INTERRUPT: u32 = 36;
pub const FUSE_BMAP: u32 = 37;
pub const FUSE_DESTROY: u32 = 38;
pub const FUSE_IOCTL: u32 = 39;
pub const FUSE_POLL: u32 = 40;
pub const FUSE_NOTIFY_REPLY: u32 = 41;
pub const FUSE_BATCH_FORGET: u32 = 42;
pub const FUSE_FALLOCATE: u32 = 43;
pub const FUSE_READDIRPLUS: u32 = 44;
pub const FUSE_RENAME2: u32 = 45;
pub const FUSE_LSEEK: u32 = 46;
pub const FUSE_COPY_FILE_RANGE: u32 = 47;
pub const FUSE_SETUPMAPPING: u32 = 48;
pub const FUSE_REMOVEMAPPING: u32 = 49;
pub const FUSE_SYNCFS: u32 = 50;
pub const FUSE_TMPFILE: u32 = 51;
pub const FUSE_STATX: u32 = 52;
pub const FUSE_COPY_FILE_RANGE_64: u32 = 53;
pub const CUSE_INIT: u32 = 4096;
pub const CUSE_INIT_BSWAP_RESERVED: u32 = 1048576;
pub const FUSE_INIT_BSWAP_RESERVED: u32 = 436207616;

pub const FUSE_NOTIFY_POLL: i32 = 1;
pub const FUSE_NOTIFY_INVAL_INODE: i32 = 2;
pub const FUSE_NOTIFY_INVAL_ENTRY: i32 = 3;
pub const FUSE_NOTIFY_STORE: i32 = 4;
pub const FUSE_NOTIFY_RETRIEVE: i32 = 5;
pub const FUSE_NOTIFY_DELETE: i32 = 6;
pub const FUSE_NOTIFY_RESEND: i32 = 7;
pub const FUSE_NOTIFY_INC_EPOCH: i32 = 8;
pub const FUSE_NOTIFY_PRUNE: i32 = 9;

pub const FUSE_ALIGNMENT: usize = size_of::<u64>();
pub const FUSE_MIN_READ_BUFFER: usize = 8192;
pub const FUSE_COMPAT_ENTRY_OUT_SIZE: u32 = 120;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_entry_out {
    pub nodeid: u64,
    pub generation: u64,
    pub entry_valid: u64,
    pub attr_valid: u64,
    pub entry_valid_nsec: u32,
    pub attr_valid_nsec: u32,
    pub attr: fuse_attr,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_forget_in {
    pub nlookup: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_forget_one {
    pub nodeid: u64,
    pub nlookup: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_batch_forget_in {
    pub count: u32,
    pub dummy: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_getattr_in {
    pub getattr_flags: u32,
    pub dummy: u32,
    pub fh: u64,
}

pub const FUSE_COMPAT_ATTR_OUT_SIZE: u32 = 96;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_attr_out {
    pub attr_valid: u64,
    pub attr_valid_nsec: u32,
    pub dummy: u32,
    pub attr: fuse_attr,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_statx_in {
    pub getattr_flags: u32,
    pub reserved: u32,
    pub fh: u64,
    pub sx_flags: u32,
    pub sx_mask: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_statx_out {
    pub attr_valid: u64,
    pub attr_valid_nsec: u32,
    pub flags: u32,
    pub spare: [u64; 2],
    pub stat: fuse_statx,
}

pub const FUSE_COMPAT_MKNOD_IN_SIZE: u32 = 8;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_mknod_in {
    pub mode: u32,
    pub rdev: u32,
    pub umask: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_mkdir_in {
    pub mode: u32,
    pub umask: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_rename_in {
    pub newdir: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_rename2_in {
    pub newdir: u64,
    pub flags: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_link_in {
    pub oldnodeid: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_setattr_in {
    pub valid: u32,
    pub padding: u32,
    pub fh: u64,
    pub size: u64,
    pub lock_owner: u64,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub atimensec: u32,
    pub mtimensec: u32,
    pub ctimensec: u32,
    pub mode: u32,
    pub unused4: u32,
    pub uid: u32,
    pub gid: u32,
    pub unused5: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_open_in {
    pub flags: u32,
    pub open_flags: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_create_in {
    pub flags: u32,
    pub mode: u32,
    pub umask: u32,
    pub open_flags: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_open_out {
    pub fh: u64,
    pub open_flags: u32,
    pub backing_id: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_release_in {
    pub fh: u64,
    pub flags: u32,
    pub release_flags: u32,
    pub lock_owner: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_flush_in {
    pub fh: u64,
    pub unused: u32,
    pub padding: u32,
    pub lock_owner: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_read_in {
    pub fh: u64,
    pub offset: u64,
    pub size: u32,
    pub read_flags: u32,
    pub lock_owner: u64,
    pub flags: u32,
    pub padding: u32,
}

pub const FUSE_COMPAT_WRITE_IN_SIZE: u32 = 24;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_write_in {
    pub fh: u64,
    pub offset: u64,
    pub size: u32,
    pub write_flags: u32,
    pub lock_owner: u64,
    pub flags: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_write_out {
    pub size: u32,
    pub padding: u32,
}

pub const FUSE_COMPAT_STATFS_SIZE: u32 = 48;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_statfs_out {
    pub st: fuse_kstatfs,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_fsync_in {
    pub fh: u64,
    pub fsync_flags: u32,
    pub padding: u32,
}

pub const FUSE_COMPAT_SETXATTR_IN_SIZE: u32 = 8;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_setxattr_in {
    pub size: u32,
    pub flags: u32,
    pub setxattr_flags: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_getxattr_in {
    pub size: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_getxattr_out {
    pub size: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_lk_in {
    pub fh: u64,
    pub owner: u64,
    pub lk: fuse_file_lock,
    pub lk_flags: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_lk_out {
    pub lk: fuse_file_lock,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_access_in {
    pub mask: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_init_in {
    pub major: u32,
    pub minor: u32,
    pub max_readahead: u32,
    pub flags: u32,
    pub flags2: u32,
    pub unused: [u32; 11],
}

pub const FUSE_COMPAT_INIT_OUT_SIZE: u32 = 8;
pub const FUSE_COMPAT_22_INIT_OUT_SIZE: u32 = 24;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_init_out {
    pub major: u32,
    pub minor: u32,
    pub max_readahead: u32,
    pub flags: u32,
    pub max_background: u16,
    pub congestion_threshold: u16,
    pub max_write: u32,
    pub time_gran: u32,
    pub max_pages: u16,
    pub map_alignment: u16,
    pub flags2: u32,
    pub max_stack_depth: u32,
    pub request_timeout: u16,
    pub unused: [u16; 11],
}

pub const CUSE_INIT_INFO_MAX: u32 = 4096;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct cuse_init_in {
    pub major: u32,
    pub minor: u32,
    pub unused: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct cuse_init_out {
    pub major: u32,
    pub minor: u32,
    pub unused: u32,
    pub flags: u32,
    pub max_read: u32,
    pub max_write: u32,
    pub dev_major: u32,
    pub dev_minor: u32,
    pub spare: [u32; 10],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_interrupt_in {
    pub unique: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_bmap_in {
    pub block: u64,
    pub blocksize: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_bmap_out {
    pub block: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_ioctl_in {
    pub fh: u64,
    pub flags: u32,
    pub cmd: u32,
    pub arg: u64,
    pub in_size: u32,
    pub out_size: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_ioctl_iovec {
    pub base: u64,
    pub len: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_ioctl_out {
    pub result: i32,
    pub flags: u32,
    pub in_iovs: u32,
    pub out_iovs: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_poll_in {
    pub fh: u64,
    pub kh: u64,
    pub flags: u32,
    pub events: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_poll_out {
    pub revents: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_poll_wakeup_out {
    pub kh: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_fallocate_in {
    pub fh: u64,
    pub offset: u64,
    pub length: u64,
    pub mode: u32,
    pub padding: u32,
}

pub const FUSE_UNIQUE_RESEND: u64 = 1 << 63;

pub const FUSE_INVALID_UIDGID: u32 = !0;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_in_header {
    pub len: u32,
    pub opcode: u32,
    pub unique: u64,
    pub nodeid: u64,
    pub uid: u32,
    pub gid: u32,
    pub pid: u32,
    pub total_extlen: u16,
    pub padding: u16,
}

static_assertions::const_assert!(size_of::<fuse_in_header>() % FUSE_ALIGNMENT == 0);

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_out_header {
    pub len: u32,
    pub error: i32,
    pub unique: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_dirent {
    pub ino: u64,
    pub off: u64,
    pub namelen: u32,
    pub type_: u32,
    pub name: [u8; 0],
}

unsafe impl Packed for fuse_dirent {}

pub const fn FUSE_REC_ALIGN(x: usize) -> usize {
    (x + FUSE_ALIGNMENT - 1) & !(FUSE_ALIGNMENT - 1)
}

const FUSE_NAME_OFFSET: usize = offset_of!(fuse_dirent, name);

pub const fn FUSE_DIRENT_ALIGN(x: usize) -> usize {
    FUSE_REC_ALIGN(x)
}

pub const fn FUSE_DIRENT_SIZE(d: &fuse_dirent) -> usize {
    FUSE_DIRENT_ALIGN(FUSE_NAME_OFFSET + d.namelen as usize)
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_direntplus {
    pub entry_out: fuse_entry_out,
    pub dirent: fuse_dirent,
}

unsafe impl Packed for fuse_direntplus {}

pub const FUSE_NAME_OFFSET_DIRENTPLUS: usize = offset_of!(fuse_direntplus, dirent.name);

pub const fn FUSE_DIRENTPLUS_SIZE(d: &fuse_direntplus) -> usize {
    fuse_direntplus_size(d.dirent.namelen as usize)
}

pub const fn fuse_direntplus_size(namelen: usize) -> usize {
    FUSE_DIRENT_ALIGN(FUSE_NAME_OFFSET_DIRENTPLUS + namelen as usize)
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_inval_inode_out {
    pub ino: u64,
    pub off: i64,
    pub len: i64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_inval_entry_out {
    pub parent: u64,
    pub namelen: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_delete_out {
    pub parent: u64,
    pub child: u64,
    pub namelen: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_store_out {
    pub nodeid: u64,
    pub offset: u64,
    pub size: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_retrieve_out {
    pub notify_unique: u64,
    pub nodeid: u64,
    pub offset: u64,
    pub size: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_retrieve_in {
    pub dummy1: u64,
    pub offset: u64,
    pub size: u32,
    pub dummy2: u32,
    pub dummy3: u64,
    pub dummy4: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_notify_prune_out {
    pub count: u32,
    pub padding: u32,
    pub spare: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_backing_map {
    pub fd: i32,
    pub flags: u32,
    pub padding: u64,
}

pub const FUSE_DEV_IOC_MAGIC: u64 = 229;
pub const FUSE_DEV_IOC_CLONE: u64 = uapi::_IOR::<u32>(FUSE_DEV_IOC_MAGIC, 0);
pub const FUSE_DEV_IOC_BACKING_OPEN: u64 = uapi::_IOW::<fuse_backing_map>(FUSE_DEV_IOC_MAGIC, 1);
pub const FUSE_DEV_IOC_BACKING_CLOSE: u64 = uapi::_IOW::<u32>(FUSE_DEV_IOC_MAGIC, 2);
pub const FUSE_DEV_IOC_SYNC_INIT: u64 = uapi::_IO(FUSE_DEV_IOC_MAGIC, 3);

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_lseek_in {
    pub fh: u64,
    pub offset: u64,
    pub whence: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_lseek_out {
    pub offset: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_copy_file_range_in {
    pub fh_in: u64,
    pub off_in: u64,
    pub nodeid_out: u64,
    pub fh_out: u64,
    pub off_out: u64,
    pub len: u64,
    pub flags: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_copy_file_range_out {
    pub bytes_copied: u64,
}

pub const FUSE_SETUPMAPPING_FLAG_WRITE: u64 = 1 << 0;
pub const FUSE_SETUPMAPPING_FLAG_READ: u64 = 1 << 1;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_setupmapping_in {
    pub fh: u64,
    pub foffset: u64,
    pub len: u64,
    pub flags: u64,
    pub moffset: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_removemapping_in {
    pub count: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_removemapping_one {
    pub moffset: u64,
    pub len: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_syncfs_in {
    pub padding: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_secctx {
    pub size: u32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_secctx_header {
    pub size: u32,
    pub nr_secctx: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_ext_header {
    pub size: u32,
    pub type_: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_supp_groups {
    pub nr_groups: u32,
    pub groups: [u32; 0],
}

pub const FUSE_URING_IN_OUT_HEADER_SZ: u32 = 128;
pub const FUSE_URING_OP_IN_OUT_SZ: u32 = 128;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_uring_ent_in_out {
    pub flags: u64,
    pub commit_id: u64,
    pub payload_sz: u32,
    pub padding: u32,
    pub reserved: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_uring_req_header {
    pub in_out: [u8; 128],
    pub op_in: [u8; 128],
    pub ring_ent_in_out: fuse_uring_ent_in_out,
}

pub const FUSE_IO_URING_CMD_INVALID: u32 = 0;
pub const FUSE_IO_URING_CMD_REGISTER: u32 = 1;
pub const FUSE_IO_URING_CMD_COMMIT_AND_FETCH: u32 = 2;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
pub struct fuse_uring_cmd_req {
    pub flags: u64,
    pub commit_id: u64,
    pub qid: u16,
    pub padding: [u8; 6],
}

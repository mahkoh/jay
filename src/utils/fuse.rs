pub mod fuse_dir;
pub mod fuse_error;
mod fuse_fusermount;
pub mod fuse_globals;
mod fuse_handler;
mod fuse_handlers;
pub mod fuse_inode;
mod fuse_inode_cache;
pub mod fuse_mgr;
pub mod fuse_mount;
mod fuse_out;
pub mod fuse_outs;
mod fuse_reg;
mod fuse_sys;
pub mod fuse_view;
pub mod fuse_views;

bitflags! {
    FuseFlags: u64;

    FUSE_ASYNC_READ           = fuse_sys::FUSE_ASYNC_READ,
    FUSE_POSIX_LOCKS          = fuse_sys::FUSE_POSIX_LOCKS,
    FUSE_FILE_OPS             = fuse_sys::FUSE_FILE_OPS,
    FUSE_ATOMIC_O_TRUNC       = fuse_sys::FUSE_ATOMIC_O_TRUNC,
    FUSE_EXPORT_SUPPORT       = fuse_sys::FUSE_EXPORT_SUPPORT,
    FUSE_BIG_WRITES           = fuse_sys::FUSE_BIG_WRITES,
    FUSE_DONT_MASK            = fuse_sys::FUSE_DONT_MASK,
    FUSE_SPLICE_WRITE         = fuse_sys::FUSE_SPLICE_WRITE,
    FUSE_SPLICE_MOVE          = fuse_sys::FUSE_SPLICE_MOVE,
    FUSE_SPLICE_READ          = fuse_sys::FUSE_SPLICE_READ,
    FUSE_FLOCK_LOCKS          = fuse_sys::FUSE_FLOCK_LOCKS,
    FUSE_HAS_IOCTL_DIR        = fuse_sys::FUSE_HAS_IOCTL_DIR,
    FUSE_AUTO_INVAL_DATA      = fuse_sys::FUSE_AUTO_INVAL_DATA,
    FUSE_DO_READDIRPLUS       = fuse_sys::FUSE_DO_READDIRPLUS,
    FUSE_READDIRPLUS_AUTO     = fuse_sys::FUSE_READDIRPLUS_AUTO,
    FUSE_ASYNC_DIO            = fuse_sys::FUSE_ASYNC_DIO,
    FUSE_WRITEBACK_CACHE      = fuse_sys::FUSE_WRITEBACK_CACHE,
    FUSE_NO_OPEN_SUPPORT      = fuse_sys::FUSE_NO_OPEN_SUPPORT,
    FUSE_PARALLEL_DIROPS      = fuse_sys::FUSE_PARALLEL_DIROPS,
    FUSE_HANDLE_KILLPRIV      = fuse_sys::FUSE_HANDLE_KILLPRIV,
    FUSE_POSIX_ACL            = fuse_sys::FUSE_POSIX_ACL,
    FUSE_ABORT_ERROR          = fuse_sys::FUSE_ABORT_ERROR,
    FUSE_MAX_PAGES            = fuse_sys::FUSE_MAX_PAGES,
    FUSE_CACHE_SYMLINKS       = fuse_sys::FUSE_CACHE_SYMLINKS,
    FUSE_NO_OPENDIR_SUPPORT   = fuse_sys::FUSE_NO_OPENDIR_SUPPORT,
    FUSE_EXPLICIT_INVAL_DATA  = fuse_sys::FUSE_EXPLICIT_INVAL_DATA,
    FUSE_MAP_ALIGNMENT        = fuse_sys::FUSE_MAP_ALIGNMENT,
    FUSE_SUBMOUNTS            = fuse_sys::FUSE_SUBMOUNTS,
    FUSE_HANDLE_KILLPRIV_V2   = fuse_sys::FUSE_HANDLE_KILLPRIV_V2,
    FUSE_SETXATTR_EXT         = fuse_sys::FUSE_SETXATTR_EXT,
    FUSE_INIT_EXT             = fuse_sys::FUSE_INIT_EXT,
    FUSE_INIT_RESERVED        = fuse_sys::FUSE_INIT_RESERVED,
    FUSE_SECURITY_CTX         = fuse_sys::FUSE_SECURITY_CTX,
    FUSE_HAS_INODE_DAX        = fuse_sys::FUSE_HAS_INODE_DAX,
    FUSE_CREATE_SUPP_GROUP    = fuse_sys::FUSE_CREATE_SUPP_GROUP,
    FUSE_HAS_EXPIRE_ONLY      = fuse_sys::FUSE_HAS_EXPIRE_ONLY,
    FUSE_DIRECT_IO_ALLOW_MMAP = fuse_sys::FUSE_DIRECT_IO_ALLOW_MMAP,
    FUSE_PASSTHROUGH          = fuse_sys::FUSE_PASSTHROUGH,
    FUSE_NO_EXPORT_SUPPORT    = fuse_sys::FUSE_NO_EXPORT_SUPPORT,
    FUSE_HAS_RESEND           = fuse_sys::FUSE_HAS_RESEND,
    FUSE_DIRECT_IO_RELAX      = fuse_sys::FUSE_DIRECT_IO_RELAX,
    FUSE_ALLOW_IDMAP          = fuse_sys::FUSE_ALLOW_IDMAP,
    FUSE_OVER_IO_URING        = fuse_sys::FUSE_OVER_IO_URING,
    FUSE_REQUEST_TIMEOUT      = fuse_sys::FUSE_REQUEST_TIMEOUT,
}

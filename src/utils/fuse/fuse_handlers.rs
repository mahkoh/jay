use crate::utils::fuse::FUSE_ABORT_ERROR;
use crate::utils::fuse::FUSE_ASYNC_DIO;
use crate::utils::fuse::FUSE_ASYNC_READ;
use crate::utils::fuse::FUSE_ATOMIC_O_TRUNC;
use crate::utils::fuse::FUSE_DO_READDIRPLUS;
use crate::utils::fuse::FUSE_INIT_EXT;
use crate::utils::fuse::fuse_dir::FuseOpenDir;
use crate::utils::fuse::fuse_inode::FuseInodeTy;
use crate::utils::fuse::fuse_inode_cache::fuse_inode_cache_types::ParentKeyRef;
use crate::utils::fuse::fuse_mgr::FuseIno;
use crate::utils::fuse::fuse_mount::FuseMountShared;
use crate::utils::fuse::fuse_outs::FileReadOut;
use crate::utils::fuse::fuse_reg::FuseOpenReg;
use crate::utils::fuse::fuse_sys::FOPEN_DIRECT_IO;
use crate::utils::fuse::fuse_sys::FOPEN_NOFLUSH;
use crate::utils::fuse::fuse_sys::FUSE_ALIGNMENT;
use crate::utils::fuse::fuse_sys::FUSE_KERNEL_VERSION;
use crate::utils::fuse::fuse_sys::FUSE_MIN_READ_BUFFER;
use crate::utils::fuse::fuse_sys::fuse_attr_out;
use crate::utils::fuse::fuse_sys::fuse_batch_forget_in;
use crate::utils::fuse::fuse_sys::fuse_entry_out;
use crate::utils::fuse::fuse_sys::fuse_forget_in;
use crate::utils::fuse::fuse_sys::fuse_forget_one;
use crate::utils::fuse::fuse_sys::fuse_in_header;
use crate::utils::fuse::fuse_sys::fuse_init_in;
use crate::utils::fuse::fuse_sys::fuse_kstatfs;
use crate::utils::fuse::fuse_sys::fuse_open_out;
use crate::utils::fuse::fuse_sys::fuse_read_in;
use crate::utils::fuse::fuse_sys::fuse_release_in;
use crate::utils::fuse::fuse_sys::fuse_statfs_out;
use crate::utils::page_size::page_size;
use crate::utils::ptr_ext::PtrExt;
use crate::utils::str_fmt::StrCtx;
use crate::utils::string_ext::StringVecExt;
use jay_algorithms::oserror::EBADF;
use jay_algorithms::oserror::EINVAL;
use jay_algorithms::oserror::ENOENT;
use jay_algorithms::oserror::ENOSYS;
use jay_algorithms::oserror::ENOTDIR;
use jay_algorithms::oserror::EPROTO;
use jay_algorithms::oserror::ESTALE;
use jay_algorithms::oserror::OsError;
use std::mem;
use std::num::NonZeroU64;
use std::ptr;
use std::rc::Rc;
use std::slice;
use uapi::Pod;
use uapi::c;

macro_rules! tail_of {
    ($ty:ty, $header:expr, $tail:expr) => {{
        const {
            assert!(size_of::<fuse_in_header>() + size_of::<$ty>() <= FUSE_MIN_READ_BUFFER);
            assert!(align_of::<$ty>() <= FUSE_ALIGNMENT);
        }
        #[allow(unused_unsafe)]
        unsafe {
            tail_of::<$ty>($header, $tail)
        }
    }};
}

impl FuseMountShared {
    fn readdir(
        &self,
        header: &fuse_in_header,
        tail: *mut u8,
        encode: impl FnOnce(&mut FuseOpenDir, usize, usize, &mut Vec<u8>),
    ) -> Result<(), OsError> {
        let tail = tail_of!(fuse_read_in, header, tail);
        let dirs = &mut *self.dirs.borrow_mut();
        let Some(dir) = dirs.get_mut(&tail.fh) else {
            return EBADF();
        };
        let mut v = self.cache.bytes();
        encode(dir, tail.offset as _, tail.size as _, &mut v.t.buf);
        self.write(header, v);
        Ok(())
    }

    fn next_fh(&self) -> u64 {
        self.fh.add_fetch(1)
    }
}

impl FuseMountShared {
    pub(super) fn handle_lookup(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        let parent = self.inodes.get(header.inode()?)?;
        let tail = unsafe {
            slice::from_raw_parts(
                tail,
                (header.len as usize - size_of_val(header)).saturating_sub(1),
            )
        };
        let Ok(tail) = str::from_utf8(tail) else {
            return ENOENT();
        };
        let Some(dirent) = parent.inode.lookup(parent.props.key, tail) else {
            return ENOENT();
        };
        let depth = parent.props.depth + 1;
        let parent = ParentKeyRef::new(parent.props.ino, dirent.static_name, tail);
        let props = self
            .inodes
            .lookup(Some(parent), depth, dirent.inode, dirent.key);
        let entry_valid = dirent.timeout_ns / 1_000_000_000;
        let entry_valid_nsec = (dirent.timeout_ns % 1_000_000_000) as u32;
        let mut v = self.cache.entry();
        v.t = fuse_entry_out {
            nodeid: props.ino.0.get(),
            attr: props.attr(),
            attr_valid: u64::MAX,
            entry_valid,
            entry_valid_nsec,
            ..uapi::pod_zeroed()
        };
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_forget(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        let tail = tail_of!(fuse_forget_in, header, tail);
        self.inodes.forget(header.inode()?, tail.nlookup);
        Ok(())
    }

    pub(super) fn handle_getattr(
        self: &Rc<Self>,
        header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        let Some(props) = self.inodes.get_props(header.inode()?) else {
            return ESTALE();
        };
        let mut v = self.cache.attr();
        v.t = fuse_attr_out {
            attr: props.attr(),
            attr_valid: u64::MAX,
            ..uapi::pod_zeroed()
        };
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_setattr(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_readlink(
        self: &Rc<Self>,
        header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        let inode = self.inodes.get(header.inode()?)?;
        let mut v = self.cache.bytes();
        let mut buf = mem::take(&mut v.t.buf).into_empty_string();
        inode
            .inode
            .readlink(inode.props.key, inode.props.depth, &mut buf);
        v.t.buf = buf.into_bytes();
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_symlink(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_mknod(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_mkdir(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_unlink(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_rmdir(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_rename(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_link(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_open(
        self: &Rc<Self>,
        header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        let inode = self.inodes.get(header.inode()?)?;
        if inode.props.ty != FuseInodeTy::Regular {
            return EINVAL();
        }
        let file = FuseOpenReg {
            inode: Rc::downgrade(&inode.inode),
            key: inode.props.key,
            ctx: StrCtx::default(),
            contents: Default::default(),
        };
        let fh = self.next_fh();
        self.files.borrow_mut().insert(fh, file);
        let mut v = self.cache.open();
        v.t = fuse_open_out {
            fh,
            open_flags: FOPEN_DIRECT_IO | FOPEN_NOFLUSH,
            backing_id: 0,
        };
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_read(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        let tail = tail_of!(fuse_read_in, header, tail);
        let files = &mut *self.files.borrow_mut();
        let Some(file) = files.get_mut(&tail.fh) else {
            return EBADF();
        };
        let contents = file.read(&self.contents)?;
        let mut v = self.cache.file_read();
        v.t = FileReadOut {
            data: Some(contents),
            offset: tail.offset as _,
            len: tail.size as _,
        };
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_write(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_statfs(
        self: &Rc<Self>,
        header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        const NAME_MAX: u32 = 255;
        let mut v = self.cache.statfs();
        v.t = fuse_statfs_out {
            st: fuse_kstatfs {
                bsize: page_size() as u32,
                frsize: page_size() as u32,
                namelen: NAME_MAX,
                ..uapi::pod_zeroed()
            },
        };
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_release(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        let tail = tail_of!(fuse_release_in, header, tail);
        let _file = self.files.borrow_mut().remove(&tail.fh);
        self.write(header, self.cache.empty());
        Ok(())
    }

    pub(super) fn handle_fsync(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_setxattr(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_getxattr(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_listxattr(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_removexattr(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_flush(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_init(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        const MINOR_VERSION: u32 = 39;
        let tail = tail_of!(fuse_init_in, header, tail);
        if tail.major < FUSE_KERNEL_VERSION {
            return EPROTO();
        }
        let mut v = self.cache.init();
        let i = &mut v.t;
        *i = uapi::pod_zeroed();
        i.major = FUSE_KERNEL_VERSION;
        i.minor = MINOR_VERSION;
        if tail.major > FUSE_KERNEL_VERSION {
            self.write(header, v);
            return Ok(());
        }
        if tail.minor < MINOR_VERSION {
            return EPROTO();
        }
        let flags = FUSE_ASYNC_READ
            | FUSE_ATOMIC_O_TRUNC
            | FUSE_DO_READDIRPLUS
            | FUSE_ASYNC_DIO
            | FUSE_ABORT_ERROR
            | FUSE_INIT_EXT;
        i.flags = flags.0 as u32;
        i.flags2 = (flags.0 >> 32) as u32;
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_opendir(
        self: &Rc<Self>,
        header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        let inode = self.inodes.get(header.inode()?)?;
        if inode.props.ty != FuseInodeTy::Directory {
            return ENOTDIR();
        }
        let dir = FuseOpenDir {
            ino: inode.props.ino,
            inode: Rc::downgrade(&inode.inode),
            key: inode.props.key,
            depth: inode.props.depth + 1,
            dirents: self.dirents.get(),
            have_dirents: false,
        };
        let fh = self.next_fh();
        self.dirs.borrow_mut().insert(fh, dir);
        let mut v = self.cache.open();
        v.t = fuse_open_out {
            fh,
            open_flags: FOPEN_DIRECT_IO | FOPEN_NOFLUSH,
            backing_id: 0,
        };
        self.write(header, v);
        Ok(())
    }

    pub(super) fn handle_readdir(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_releasedir(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        let tail = tail_of!(fuse_release_in, header, tail);
        let _dir = self.dirs.borrow_mut().remove(&tail.fh);
        self.write(header, self.cache.empty());
        Ok(())
    }

    pub(super) fn handle_fsyncdir(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_getlk(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_setlk(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_setlkw(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_access(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_create(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_interrupt(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_bmap(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_destroy(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_ioctl(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_poll(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_notify_reply(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_batch_forget(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        let tail = unsafe {
            let batch = tail_of!(fuse_batch_forget_in, header, tail);
            let tail_size = header.len as usize - size_of_val(header);
            if tail_size.saturating_sub(size_of_val(batch)) / size_of::<fuse_forget_one>()
                < batch.count as usize
            {
                return EPROTO();
            }
            let tail = tail.add(size_of_val(batch));
            slice::from_raw_parts(tail.cast::<fuse_forget_one>(), batch.count as usize)
        };
        for one in tail {
            if let Some(ino) = NonZeroU64::new(one.nodeid) {
                self.inodes.forget(FuseIno(ino), one.nlookup);
            }
        }
        Ok(())
    }

    pub(super) fn handle_fallocate(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_readdirplus(
        self: &Rc<Self>,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        self.readdir(header, tail, |dir, offset, size, buf| {
            dir.encode_plus(&self.inodes, offset, size, buf);
        })
    }

    pub(super) fn handle_rename2(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_lseek(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_copy_file_range(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_setupmapping(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_removemapping(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_syncfs(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_tmpfile(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_statx(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }

    pub(super) fn handle_copy_file_range_64(
        self: &Rc<Self>,
        _header: &fuse_in_header,
        _tail: *mut u8,
    ) -> Result<(), OsError> {
        ENOSYS()
    }
}

impl fuse_in_header {
    pub(super) fn inode(&self) -> Result<FuseIno, OsError> {
        Ok(FuseIno(NonZeroU64::new(self.nodeid).ok_or(c::EINVAL)?))
    }
}

unsafe fn tail_of<T>(header: &fuse_in_header, tail: *mut u8) -> &T
where
    T: Pod,
{
    let tail_len = header.len as usize - size_of_val(header);
    let expected = size_of::<T>();
    if tail_len < expected {
        unsafe {
            ptr::write_bytes(tail.add(tail_len), 0, expected - tail_len);
        }
    }
    unsafe { tail.cast::<T>().deref() }
}

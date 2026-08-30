use crate::io_uring::IoUringError;
use crate::io_uring::ReadMultishotCallback;
use crate::io_uring::buffer_ring::BufferRingBuffer;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_mount::FuseMountShared;
use crate::utils::fuse::fuse_out::Fov;
use crate::utils::fuse::fuse_out::FuseOut;
use crate::utils::fuse::fuse_sys;
use crate::utils::fuse::fuse_sys::fuse_in_header;
use crate::utils::ptr_ext::PtrExt;
use jay_algorithms::oserror::ENOSYS;
use jay_algorithms::oserror::EPROTO;
use jay_algorithms::oserror::OsError;
use std::rc::Rc;

impl ReadMultishotCallback for FuseMountShared {
    fn read(self: Rc<Self>, res: BufferRingBuffer) {
        self.handle_request(res);
    }

    fn done(self: Rc<Self>, res: Result<Option<BufferRingBuffer>, OsError>) {
        match res {
            Ok(res) => {
                if let Some(res) = res {
                    self.handle_request(res);
                }
                if let Err(e) = self.schedule_read_requests() {
                    self.fatal(FuseError::ScheduleReadRequest(e));
                }
            }
            Err(e) => {
                self.fatal(FuseError::ReadRequests(e));
            }
        }
    }
}

impl FuseMountShared {
    pub(super) fn fatal(&self, error: FuseError) {
        self.early.fatal(error);
    }

    pub(super) fn schedule_read_requests(self: &Rc<Self>) -> Result<(), IoUringError> {
        if self.early.cancelled.get() {
            return Ok(());
        }
        let pending = self
            .ring
            .read_multishot(&self.fd, &self.buffer_ring, self.clone())?;
        self.early.read_multishot.set(Some(pending));
        Ok(())
    }

    pub(super) fn write_err(&self, header: &fuse_in_header, err: OsError) {
        let mut v = self.cache.error();
        v.header.error = -err.0;
        self.write(header, v);
    }

    pub(super) fn write<T>(&self, header: &fuse_in_header, mut v: Box<Fov<T>>)
    where
        T: FuseOut,
    {
        v.header.unique = header.unique;
        v.set_len();
        if let Err(e) = self.ring.writev_external(&self.fd, v) {
            log::error!("Could not write fuse message: {}", ErrorFmt(e));
        }
    }
}

impl FuseMountShared {
    pub(super) fn handle_request(self: &Rc<Self>, res: BufferRingBuffer) {
        if res.len() < size_of::<fuse_in_header>() {
            return;
        }
        let ptr = res.as_mut_ptr();
        let header = unsafe { ptr.cast::<fuse_in_header>().deref() };
        let tail = unsafe { ptr.add(size_of::<fuse_in_header>()) };
        if let Err(e) = self.handle_request_(&res, header, tail) {
            self.write_err(header, e);
        }
    }

    fn handle_request_(
        self: &Rc<Self>,
        res: &BufferRingBuffer,
        header: &fuse_in_header,
        tail: *mut u8,
    ) -> Result<(), OsError> {
        if header.len as usize != res.len() {
            return EPROTO();
        }
        match header.opcode {
            fuse_sys::FUSE_LOOKUP => self.handle_lookup(header, tail),
            fuse_sys::FUSE_FORGET => self.handle_forget(header, tail),
            fuse_sys::FUSE_GETATTR => self.handle_getattr(header, tail),
            fuse_sys::FUSE_SETATTR => self.handle_setattr(header, tail),
            fuse_sys::FUSE_READLINK => self.handle_readlink(header, tail),
            fuse_sys::FUSE_SYMLINK => self.handle_symlink(header, tail),
            fuse_sys::FUSE_MKNOD => self.handle_mknod(header, tail),
            fuse_sys::FUSE_MKDIR => self.handle_mkdir(header, tail),
            fuse_sys::FUSE_UNLINK => self.handle_unlink(header, tail),
            fuse_sys::FUSE_RMDIR => self.handle_rmdir(header, tail),
            fuse_sys::FUSE_RENAME => self.handle_rename(header, tail),
            fuse_sys::FUSE_LINK => self.handle_link(header, tail),
            fuse_sys::FUSE_OPEN => self.handle_open(header, tail),
            fuse_sys::FUSE_READ => self.handle_read(header, tail),
            fuse_sys::FUSE_WRITE => self.handle_write(header, tail),
            fuse_sys::FUSE_STATFS => self.handle_statfs(header, tail),
            fuse_sys::FUSE_RELEASE => self.handle_release(header, tail),
            fuse_sys::FUSE_FSYNC => self.handle_fsync(header, tail),
            fuse_sys::FUSE_SETXATTR => self.handle_setxattr(header, tail),
            fuse_sys::FUSE_GETXATTR => self.handle_getxattr(header, tail),
            fuse_sys::FUSE_LISTXATTR => self.handle_listxattr(header, tail),
            fuse_sys::FUSE_REMOVEXATTR => self.handle_removexattr(header, tail),
            fuse_sys::FUSE_FLUSH => self.handle_flush(header, tail),
            fuse_sys::FUSE_INIT => self.handle_init(header, tail),
            fuse_sys::FUSE_OPENDIR => self.handle_opendir(header, tail),
            fuse_sys::FUSE_READDIR => self.handle_readdir(header, tail),
            fuse_sys::FUSE_RELEASEDIR => self.handle_releasedir(header, tail),
            fuse_sys::FUSE_FSYNCDIR => self.handle_fsyncdir(header, tail),
            fuse_sys::FUSE_GETLK => self.handle_getlk(header, tail),
            fuse_sys::FUSE_SETLK => self.handle_setlk(header, tail),
            fuse_sys::FUSE_SETLKW => self.handle_setlkw(header, tail),
            fuse_sys::FUSE_ACCESS => self.handle_access(header, tail),
            fuse_sys::FUSE_CREATE => self.handle_create(header, tail),
            fuse_sys::FUSE_INTERRUPT => self.handle_interrupt(header, tail),
            fuse_sys::FUSE_BMAP => self.handle_bmap(header, tail),
            fuse_sys::FUSE_DESTROY => self.handle_destroy(header, tail),
            fuse_sys::FUSE_IOCTL => self.handle_ioctl(header, tail),
            fuse_sys::FUSE_POLL => self.handle_poll(header, tail),
            fuse_sys::FUSE_NOTIFY_REPLY => self.handle_notify_reply(header, tail),
            fuse_sys::FUSE_BATCH_FORGET => self.handle_batch_forget(header, tail),
            fuse_sys::FUSE_FALLOCATE => self.handle_fallocate(header, tail),
            fuse_sys::FUSE_READDIRPLUS => self.handle_readdirplus(header, tail),
            fuse_sys::FUSE_RENAME2 => self.handle_rename2(header, tail),
            fuse_sys::FUSE_LSEEK => self.handle_lseek(header, tail),
            fuse_sys::FUSE_COPY_FILE_RANGE => self.handle_copy_file_range(header, tail),
            fuse_sys::FUSE_SETUPMAPPING => self.handle_setupmapping(header, tail),
            fuse_sys::FUSE_REMOVEMAPPING => self.handle_removemapping(header, tail),
            fuse_sys::FUSE_SYNCFS => self.handle_syncfs(header, tail),
            fuse_sys::FUSE_TMPFILE => self.handle_tmpfile(header, tail),
            fuse_sys::FUSE_STATX => self.handle_statx(header, tail),
            fuse_sys::FUSE_COPY_FILE_RANGE_64 => self.handle_copy_file_range_64(header, tail),
            _ => ENOSYS(),
        }
    }
}

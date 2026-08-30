use crate::async_engine::AsyncEngine;
use crate::async_engine::SpawnedFuture;
use crate::forker::ForkerProxy;
use crate::io_uring::IoUring;
use crate::utils::buf::Buf;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::fuse::fuse_inode::FuseInodeTy;
use crate::utils::fuse::fuse_inode_cache::InodeCache;
use crate::utils::fuse::fuse_mgr::FuseMgr;
use crate::utils::fuse::fuse_mount::FuseMountEarlyShared;
use crate::utils::fuse::fuse_mount::FuseMountOwnerHolder;
use crate::utils::fuse::fuse_mount::FuseMountShared;
use crate::utils::fuse::fuse_sys::FUSE_ALIGNMENT;
use crate::utils::fuse::fuse_sys::FUSE_MIN_READ_BUFFER;
use crate::utils::fuse::fuse_sys::FUSE_ROOT_ID;
use crate::utils::line_logger::log_lines;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use bstr::ByteSlice;
use jay_algorithms::oserror::OsErrorExt2;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::future::pending;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::rc::Weak;
use std::slice;
use uapi::c;

impl FuseMgr {
    pub(super) fn fusermount(
        &self,
        forker: Option<Rc<ForkerProxy>>,
        shared: &Rc<FuseMountEarlyShared>,
        path: &str,
        owner: Rc<FuseMountOwnerHolder>,
        root: Weak<dyn FuseInode>,
    ) -> SpawnedFuture<()> {
        let fut = fusermount(
            shared.clone(),
            self.eng.clone(),
            self.ring.clone(),
            forker,
            path.to_string(),
            owner,
            root,
        );
        self.eng.spawn("fusermount", fut)
    }
}

const FUSE_COMMFD_ENV: &str = "_FUSE_COMMFD";
const FUSERMOUNT3: &str = "fusermount3";

async fn fusermount(
    shared: Rc<FuseMountEarlyShared>,
    eng: Rc<AsyncEngine>,
    ring: Rc<IoUring>,
    forker: Option<Rc<ForkerProxy>>,
    path: String,
    holder: Rc<FuseMountOwnerHolder>,
    root: Weak<dyn FuseInode>,
) {
    let res = try_fusermount(&shared, &eng, &ring, forker, &path, &holder, root).await;
    holder.error.set(Some(res.unwrap_err()));
}

async fn try_fusermount(
    shared: &Rc<FuseMountEarlyShared>,
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
    forker: Option<Rc<ForkerProxy>>,
    path: &str,
    holder: &Rc<FuseMountOwnerHolder>,
    root: Weak<dyn FuseInode>,
) -> Result<Infallible, FuseError> {
    let inodes = InodeCache::new()?;
    let Some(root) = root.upgrade() else {
        return Err(FuseError::RootInodeDropped);
    };
    let root = inodes.lookup(None, 0, root, 0);
    if root.ino.0.get() != FUSE_ROOT_ID {
        return Err(FuseError::RootInodeId);
    }
    if root.ty != FuseInodeTy::Directory {
        return Err(FuseError::RootInodeDir);
    }
    let Some(forker) = forker else {
        return Err(FuseError::NoForker);
    };
    let (p, c) = uapi::socketpair(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0)
        .map_os_err(FuseError::CreateSocketpair)?;
    let Pipe { read, write } = pipe()
        .map_err(FuseError::CreatePipe)?
        .map_read(Rc::new)
        .map_write(Rc::new);
    let log_stderr = {
        let ring = ring.clone();
        async move {
            let res = log_lines(&ring, &read, |a, b| {
                log::info!("fusermount3: {}{}", a.as_bstr(), b.as_bstr());
            })
            .await;
            if let Err(e) = res {
                log::error!("Could not read from fusermount3 pipe: {}", ErrorFmt(e));
            }
        }
    };
    let _log_stderr = eng.spawn("fusermount3 stderr", log_stderr);
    let args = vec![
        "-o".to_string(),
        "rw,noatime,auto_unmount,default_permissions,fsname=jay-fuse".to_string(),
        path.to_string(),
    ];
    let env = vec![(FUSE_COMMFD_ENV.to_string(), Some("3".to_string()))];
    let fds = vec![(2, write), (3, Rc::new(c))];
    forker.spawn(FUSERMOUNT3.to_string(), args, env, fds);
    let mut buf = Buf::new(1);
    let p = Rc::new(p);
    let mut fds = VecDeque::new();
    ring.recvmsg(&p, slice::from_mut(&mut buf), &mut fds)
        .await
        .map_err(FuseError::Recvmsg)?;
    let fd = fds.pop_front().ok_or(FuseError::NoFd)?;
    let buffer_ring = ring
        .create_buffer_ring(16, FUSE_MIN_READ_BUFFER, FUSE_ALIGNMENT)
        .map_err(FuseError::CreateBufferRing)?;
    let shared = Rc::new(FuseMountShared {
        eng: eng.clone(),
        ring: ring.clone(),
        _socket: p,
        fd,
        buffer_ring,
        cache: Default::default(),
        inodes: ManuallyDrop::new(inodes),
        early: shared.clone(),
        fh: Default::default(),
        dirs: Default::default(),
        dirents: Default::default(),
        files: Default::default(),
        contents: Default::default(),
    });
    shared
        .schedule_read_requests()
        .map_err(FuseError::ScheduleReadRequest)?;
    let _prune = eng.spawn("fuse-prune", shared.clone().periodic_prune());
    holder.success();
    let _ = ring.poll(&shared.fd, 0).await;
    shared.fatal(FuseError::Aborted);
    pending().await
}

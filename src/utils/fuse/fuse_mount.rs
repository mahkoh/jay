use crate::async_engine::AsyncEngine;
use crate::async_engine::SpawnedFuture;
use crate::io_uring::IoUring;
use crate::io_uring::PendingReadMultishot;
use crate::io_uring::buffer_ring::BufferRing;
use crate::utils::box_cache::BoxCache;
use crate::utils::box_cache::BoxReset;
use crate::utils::clonecell::CloneCell;
use crate::utils::fuse::fuse_dir::FuseDirents;
use crate::utils::fuse::fuse_dir::FuseOpenDir;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_inode_cache::InodeCache;
use crate::utils::fuse::fuse_mgr::FuseMgrShutdownQueue;
use crate::utils::fuse::fuse_out::OutCache;
use crate::utils::fuse::fuse_reg::FuseOpenReg;
use crate::utils::fx_hash::FxBuildHasher;
use crate::utils::numcell::NumCell;
use crate::utils::run_toplevel::RunToplevel;
use hashbrown::HashMap;
use jay_algorithms::oserror::OsError;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::rc::Weak;
use uapi::OwnedFd;
use uapi::c;

pub trait FuseMountOwner {
    fn success(&self);
    fn failed(self: Rc<Self>, error: FuseError);
}

pub struct FuseMount {
    pub(super) shared: Rc<FuseMountEarlyShared>,
}

pub(super) struct FuseMountOwnerHolder {
    pub(super) run_toplevel: Rc<RunToplevel>,
    pub(super) owner: Rc<CloneCell<Option<Rc<dyn FuseMountOwner>>>>,
    pub(super) error: Cell<Option<FuseError>>,
}

pub(super) struct FuseMountEarlyShared {
    pub(super) shutdown: Rc<FuseMgrShutdownQueue>,
    pub(super) read_multishot: Cell<Option<PendingReadMultishot>>,
    pub(super) futures: Cell<Vec<SpawnedFuture<()>>>,
    pub(super) cancelled: Cell<bool>,
    pub(super) owner: Weak<FuseMountOwnerHolder>,
}

pub(super) struct FuseMountShared {
    pub(super) eng: Rc<AsyncEngine>,
    pub(super) ring: Rc<IoUring>,
    pub(super) _socket: Rc<OwnedFd>,
    pub(super) fd: Rc<OwnedFd>,
    pub(super) buffer_ring: Rc<BufferRing>,
    pub(super) cache: Rc<OutCache>,
    pub(super) inodes: ManuallyDrop<InodeCache>,
    pub(super) early: Rc<FuseMountEarlyShared>,
    pub(super) fh: NumCell<u64>,
    pub(super) dirs: RefCell<HashMap<u64, FuseOpenDir, FxBuildHasher>>,
    pub(super) dirents: Rc<BoxCache<FuseDirents, BoxReset>>,
    pub(super) files: RefCell<HashMap<u64, FuseOpenReg, FxBuildHasher>>,
    pub(super) contents: Rc<BoxCache<String, BoxReset>>,
}

impl Drop for FuseMountShared {
    fn drop(&mut self) {
        self.cache.clear();
        let inodes = unsafe { ManuallyDrop::take(&mut self.inodes) };
        self.early.shutdown.push(inodes);
    }
}

impl FuseMountEarlyShared {
    pub(super) fn clear(&self) {
        if let Some(v) = self.owner.upgrade() {
            v.owner.take();
        }
        self.cancel();
    }

    pub(super) fn fatal(&self, mut error: FuseError) {
        if let FuseError::ReadRequests(OsError(c::ECONNABORTED)) = error {
            error = FuseError::Aborted;
        }
        if let Some(v) = self.owner.upgrade() {
            v.error.set(Some(error));
        }
        self.cancel();
    }

    fn cancel(&self) {
        self.cancelled.set(true);
        self.futures.take();
        self.read_multishot.take();
    }
}

impl FuseMountOwnerHolder {
    pub(super) fn success(&self) {
        let owner = self.owner.clone();
        self.run_toplevel.schedule(move || {
            if let Some(owner) = owner.get() {
                owner.success();
            }
        });
    }
}

impl Drop for FuseMountOwnerHolder {
    fn drop(&mut self) {
        let error = self.error.take().unwrap_or(FuseError::Unknown);
        let owner = self.owner.clone();
        self.run_toplevel.schedule(move || {
            if let Some(owner) = owner.take() {
                owner.failed(error);
            }
        });
    }
}

impl Drop for FuseMount {
    fn drop(&mut self) {
        self.shared.clear();
    }
}

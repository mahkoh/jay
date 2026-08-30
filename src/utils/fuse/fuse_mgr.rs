use crate::async_engine::AsyncEngine;
use crate::async_engine::SpawnedFuture;
use crate::forker::ForkerProxy;
use crate::io_uring::IoUring;
use crate::state::State;
use crate::utils::clonecell::CloneCell;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::fuse::fuse_inode_cache::InodeCache;
use crate::utils::fuse::fuse_mount::FuseMount;
use crate::utils::fuse::fuse_mount::FuseMountEarlyShared;
use crate::utils::fuse::fuse_mount::FuseMountOwner;
use crate::utils::fuse::fuse_mount::FuseMountOwnerHolder;
use crate::utils::queue::AsyncQueue;
use crate::utils::run_toplevel::RunToplevel;
use std::cell::Cell;
use std::num::NonZeroU64;
use std::rc::Rc;

#[jay_proc::jay_hash]
#[derive(Copy, Clone, Debug, Eq, Ord, PartialOrd)]
pub(super) struct FuseIno(pub(super) NonZeroU64);

pub struct FuseMgr {
    pub(super) run_toplevel: Rc<RunToplevel>,
    pub(super) eng: Rc<AsyncEngine>,
    pub(super) ring: Rc<IoUring>,
    pub(super) shutdown: Rc<FuseMgrShutdownQueue>,
    pub(super) _task: SpawnedFuture<()>,
}

pub(super) struct FuseMgrShutdownQueue {
    ring: Rc<IoUring>,
    dropped: Cell<bool>,
    queue: AsyncQueue<InodeCache>,
}

impl FuseMgrShutdownQueue {
    async fn run(self: Rc<Self>) {
        loop {
            self.queue.pop().await.shutdown(&self.ring).await;
        }
    }

    pub(super) fn push(&self, cache: InodeCache) {
        if self.dropped.get() {
            return;
        }
        self.queue.push(cache);
    }
}

impl FuseMgr {
    pub fn new(eng: &Rc<AsyncEngine>, ring: &Rc<IoUring>, run_toplevel: &Rc<RunToplevel>) -> Self {
        let shutdown = Rc::new(FuseMgrShutdownQueue {
            ring: ring.clone(),
            dropped: Default::default(),
            queue: Default::default(),
        });
        let task = eng.spawn("fuse shutdown queue", shutdown.clone().run());
        Self {
            run_toplevel: run_toplevel.clone(),
            eng: eng.clone(),
            ring: ring.clone(),
            shutdown,
            _task: task,
        }
    }

    pub fn mount(
        &self,
        forker: Option<Rc<ForkerProxy>>,
        owner: Rc<dyn FuseMountOwner>,
        root: Rc<dyn FuseInode>,
        path: &str,
    ) -> FuseMount {
        let owner = Rc::new(FuseMountOwnerHolder {
            run_toplevel: self.run_toplevel.clone(),
            owner: Rc::new(CloneCell::new(Some(owner))),
            error: Default::default(),
        });
        let shared = Rc::new(FuseMountEarlyShared {
            shutdown: self.shutdown.clone(),
            read_multishot: Default::default(),
            futures: Default::default(),
            cancelled: Default::default(),
            owner: Rc::downgrade(&owner),
        });
        let future = self.fusermount(forker, &shared, path, owner, Rc::downgrade(&root));
        shared.futures.set(vec![future]);
        FuseMount { shared }
    }
}

impl State {
    #[expect(unused)]
    pub fn fuse_mount(
        &self,
        owner: Rc<dyn FuseMountOwner>,
        root: Rc<dyn FuseInode>,
        path: &str,
    ) -> FuseMount {
        self.fuse.mount(self.forker.get(), owner, root, path)
    }
}

impl Drop for FuseMgr {
    fn drop(&mut self) {
        self.shutdown.dropped.set(true);
        self.shutdown.queue.clear();
    }
}

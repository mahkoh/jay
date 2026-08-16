use crate::io_uring::FutexObj;
use crate::io_uring::IoUring;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::fuse::fuse_inode::FuseInodeProps;
use crate::utils::fuse::fuse_inode::FuseInodePropsPti;
use crate::utils::fuse::fuse_inode::FuseInodeTy;
use crate::utils::fuse::fuse_inode_cache::fuse_inode_cache_types::FuseInodeKey;
use crate::utils::fuse::fuse_inode_cache::fuse_inode_cache_types::FuseInodeKeyRef;
use crate::utils::fuse::fuse_inode_cache::fuse_inode_cache_types::ParentKey;
use crate::utils::fuse::fuse_inode_cache::fuse_inode_cache_types::ParentKeyRef;
use crate::utils::fuse::fuse_mgr::FuseIno;
use crate::utils::fuse::fuse_mount::FuseMountShared;
use crate::utils::fuse::fuse_sys::fuse_attr;
use crate::utils::futex::futex_wait;
use crate::utils::futex::futex_wake;
use crate::utils::fx_hash::FxBuildHasher;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::liveness::LivenessView;
use crate::utils::numcell::NumCell;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::send_sync_ptr::SendSyncPtrConst;
use derivative::Derivative;
use futures_util::future::Either;
use futures_util::future::select;
use hashbrown::HashMap;
use hashbrown::hash_map::OccupiedEntry;
use hashbrown::hash_map::RawEntryMut;
use isnt::std_1::ops::IsntRangeExt;
use jay_algorithms::oserror::OsError;
use std::any::TypeId;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::BuildHasher;
use std::num::NonZeroU64;
use std::ops::Deref;
use std::ops::Range;
use std::pin::pin;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Acquire;
use std::sync::atomic::Ordering::Release;
use std::thread;
use std::thread::JoinHandle;
use uapi::OwnedFd;
use uapi::c;

pub mod fuse_inode_cache_types;

pub(super) struct InodeCache {
    next_ino: NumCell<u64>,
    thread: Cell<Option<JoinHandle<()>>>,
    thread_pipe: Rc<OwnedFd>,
    files: RefCell<CachedFiles>,
    data: Rc<Data>,
    prune: AsyncEvent,
}

impl InodeCache {
    pub fn new() -> Result<Self, FuseError> {
        let Pipe { read, write } = pipe().map_err(FuseError::CreatePipe)?.map_write(Rc::new);
        let data = Rc::new(Data::default());
        let thread = {
            let data = SendSyncPtrConst::<Data>(&*data);
            thread::Builder::new()
                .name("prune-inodes".to_string())
                .spawn(move || {
                    let _read = read;
                    let data = data;
                    let data = unsafe { &*data.0 };
                    handle_prune(data);
                })
                .map_err(FuseError::SpawnPruneThread)?
        };
        Ok(Self {
            next_ino: NumCell::new(1),
            thread: Cell::new(Some(thread)),
            thread_pipe: write,
            files: Default::default(),
            data,
            prune: Default::default(),
        })
    }
}

#[derive(Default)]
struct Data {
    is_locked: Cell<bool>,
    files: RefCell<SharedFiles>,
    ts: AtomicU32,
}

impl FutexObj for Data {
    fn get(&self) -> &AtomicU32 {
        &self.ts
    }
}

const TS_PRUNED: u32 = 0;
const TS_PRUNE: u32 = 1;
const TS_EXIT: u32 = 2;

#[derive(Derivative)]
#[derivative(Default)]
struct CachedFiles {
    #[derivative(Default(value = "0..64"))]
    len_range: Range<usize>,
    inos: HashMap<FuseInodeKey, FuseIno, FxBuildHasher>,
    inodes: HashMap<FuseIno, CachedInode, FxBuildHasher>,
    todo: VecDeque<Todo>,
}

enum Todo {
    Add(FuseIno, LivenessView),
    Del(FuseIno),
}

#[derive(Derivative)]
#[derivative(Default)]
struct SharedFiles {
    liveness: HashMap<FuseIno, LivenessView, FxBuildHasher>,
    dead: Vec<FuseIno>,
}

struct CachedInode {
    inode: Weak<dyn FuseInode>,
    props: FuseInodePropsExt,
    parent: Option<ParentKey>,
    lookups: u64,
}

pub(super) struct FoundInode {
    pub(super) inode: Rc<dyn FuseInode>,
    pub(super) props: FuseInodePropsExt,
}

#[derive(Copy, Clone)]
pub(super) struct FuseInodePropsExt {
    pub(super) props: FuseInodeProps,
    pub(super) ino: FuseIno,
    pub(super) key: u64,
    pub(super) depth: u64,
    type_id: TypeId,
}

impl Deref for FuseInodePropsExt {
    type Target = FuseInodeProps;

    fn deref(&self) -> &Self::Target {
        &self.props
    }
}

impl InodeCache {
    pub(super) fn get(&self, id: FuseIno) -> Result<FoundInode, OsError> {
        self.get_(id).ok_or(OsError(c::ESTALE))
    }

    fn get_(&self, id: FuseIno) -> Option<FoundInode> {
        let files = &mut *self.files.borrow_mut();
        let entry = files.inodes.occupied_entry(&id)?;
        let cached = entry.get();
        let Some(inode) = cached.inode.upgrade() else {
            self.remove(id, &mut files.inos, &mut files.todo, entry);
            self.maybe_prune(files);
            return None;
        };
        Some(FoundInode {
            inode,
            props: cached.props,
        })
    }

    pub(super) fn get_props(&self, id: FuseIno) -> Option<FuseInodePropsExt> {
        let files = &mut *self.files.borrow_mut();
        let entry = files.inodes.occupied_entry(&id)?;
        let cached = entry.get();
        if cached.inode.strong_count() == 0 {
            self.remove(id, &mut files.inos, &mut files.todo, entry);
            self.maybe_prune(files);
            return None;
        }
        Some(cached.props)
    }

    pub(super) fn forget(&self, id: FuseIno, count: u64) {
        let _entry = {
            let files = &mut *self.files.borrow_mut();
            let Some(mut entry) = files.inodes.occupied_entry(&id) else {
                return;
            };
            let cached = entry.get_mut();
            cached.lookups = cached.lookups.saturating_sub(count);
            if cached.lookups == 0 {
                let cached = self.remove(id, &mut files.inos, &mut files.todo, entry);
                self.maybe_prune(files);
                Some(cached)
            } else {
                None
            }
        };
    }

    pub(super) fn lookup(
        &self,
        parent: Option<ParentKeyRef<'_>>,
        depth: u64,
        inode: Rc<dyn FuseInode>,
        key: u64,
    ) -> FuseInodePropsExt {
        let props = inode.props_pti(key);
        let addr = Rc::as_ptr(&inode).addr();
        let inode = || Rc::downgrade(&inode);
        self.lookup2(parent, depth, addr, inode, key, props)
    }

    pub(super) fn lookup2(
        &self,
        mut parent: Option<ParentKeyRef<'_>>,
        mut depth: u64,
        addr: usize,
        inode: impl FnOnce() -> Weak<dyn FuseInode>,
        key: u64,
        props: FuseInodePropsPti,
    ) -> FuseInodePropsExt {
        let FuseInodePropsPti {
            props,
            type_id,
            liveness,
        } = props;
        if props.ty != FuseInodeTy::Directory {
            parent = None;
        }
        if props.ty == FuseInodeTy::Regular {
            depth = 0;
        }
        let fik = FuseInodeKeyRef {
            parent,
            type_id,
            addr,
            key,
            depth,
        };
        let files = &mut *self.files.borrow_mut();
        let hash = files.inos.hasher().hash_one(&fik);
        let entry = files
            .inos
            .raw_entry_mut()
            .from_key_hashed_nocheck(hash, &fik);
        let (inode_key, &mut ino) = match entry {
            RawEntryMut::Occupied(v) => v.into_key_value(),
            RawEntryMut::Vacant(v) => {
                v.insert_hashed_nocheck(hash, fik.to_owned(), self.next_ino())
            }
        };
        let props = FuseInodePropsExt {
            props,
            ino,
            key,
            depth,
            type_id,
        };
        let cached = files.inodes.entry(ino).or_insert_with(|| {
            if self.data.is_locked.get() {
                files.todo.push_back(Todo::Add(ino, liveness));
            } else {
                self.data.files.borrow_mut().liveness.insert(ino, liveness);
            }
            CachedInode {
                props,
                parent: inode_key.parent.clone(),
                lookups: Default::default(),
                inode: inode(),
            }
        });
        cached.lookups += 1;
        if cached.lookups == 1 {
            self.maybe_prune(files);
        }
        props
    }

    pub(super) fn next_ino(&self) -> FuseIno {
        unsafe { FuseIno(NonZeroU64::new_unchecked(self.next_ino.fetch_add(1))) }
    }

    fn maybe_prune(&self, files: &mut CachedFiles) {
        if files.len_range.not_contains(&files.inodes.len()) {
            self.prune.trigger();
        }
    }

    fn remove(
        &self,
        ino: FuseIno,
        inos: &mut HashMap<FuseInodeKey, FuseIno, FxBuildHasher>,
        todos: &mut VecDeque<Todo>,
        entry: OccupiedEntry<'_, FuseIno, CachedInode, FxBuildHasher>,
    ) -> CachedInode {
        let cached = entry.remove();
        let key = cached.key();
        inos.remove(&key);
        if self.data.is_locked.get() {
            todos.push_back(Todo::Del(ino));
        } else {
            self.data.files.borrow_mut().liveness.remove(&ino);
        }
        cached
    }
}

impl FuseInodePropsExt {
    pub(super) fn attr(&self) -> fuse_attr {
        self.props.attr(Some(self.ino))
    }
}

impl FuseMountShared {
    pub(super) async fn periodic_prune(self: Rc<Self>) {
        if let Err(e) = self.try_periodic_prune().await {
            self.fatal(e);
        }
    }

    async fn try_periodic_prune(self: &Rc<Self>) -> Result<(), FuseError> {
        const SCHEDULE: u64 = 5 * 60 * 1_000_000_000;
        let inodes = &self.inodes;
        let data = &*inodes.data;
        loop {
            let next = self.eng.now().nsec() + SCHEDULE;
            let timeout = pin!(self.ring.timeout(next));
            let res = select(inodes.prune.triggered(), timeout).await;
            if let Either::Right((Err(e), _)) = res {
                return Err(FuseError::Sleep(e));
            }
            data.is_locked.set(true);
            data.ts.store(TS_PRUNE, Release);
            self.ring
                .futex_wake(&inodes.data, i32::MAX, true)
                .map_err(FuseError::FutexWake)?;
            loop {
                let ts = data.ts.load(Acquire);
                if ts == TS_PRUNED {
                    break;
                }
                self.ring
                    .futex_wait(&inodes.data, ts, true)
                    .await
                    .map_err(FuseError::FutexWait)?;
            }
            let shared = &mut *data.files.borrow_mut();
            let files = &mut *inodes.files.borrow_mut();
            let old = files.inodes.len();
            while let Some(ino) = shared.dead.pop() {
                if let Some(cached) = files.inodes.remove(&ino) {
                    files.inos.remove(&cached.key());
                }
            }
            let new = files.inodes.len();
            let pruned = old - new;
            if pruned > 0 {
                log::debug!("Pruned {pruned} entries: {old} -> {new}");
            }
            while let Some(todo) = files.todo.pop_front() {
                match todo {
                    Todo::Add(ino, liveness) => {
                        shared.liveness.insert(ino, liveness);
                    }
                    Todo::Del(ino) => {
                        shared.liveness.remove(&ino);
                    }
                }
            }
            let lo;
            let hi;
            if new < 64 {
                lo = 0;
                hi = new.max(32) * 2;
            } else {
                lo = new / 2;
                hi = new * 2;
            }
            files.len_range = lo..hi;
            inodes.prune.reset_triggers();
            data.is_locked.set(false);
        }
    }
}

fn handle_prune(data: &Data) {
    loop {
        match data.ts.load(Acquire) {
            TS_PRUNED => {
                let _ = futex_wait(&data.ts, TS_PRUNED);
            }
            TS_PRUNE => {
                let files = unsafe { data.files.as_ptr().deref_mut() };
                let iter = files.liveness.extract_if(|_, v| v.is_dead());
                for (ino, _) in iter {
                    files.dead.push(ino);
                }
                data.ts.store(TS_PRUNED, Release);
                let _ = futex_wake(&data.ts);
            }
            TS_EXIT => {
                return;
            }
            _ => {
                unreachable!();
            }
        }
    }
}

impl Drop for InodeCache {
    fn drop(&mut self) {
        let _ = futex_wake(&self.data.ts);
        loop {
            let val = self.data.ts.load(Acquire);
            if val == TS_PRUNED || val == TS_EXIT {
                break;
            }
            let _ = futex_wait(&self.data.ts, val);
        }
        self.data.ts.store(TS_EXIT, Release);
        let _ = futex_wake(&self.data.ts);
        self.thread.take().unwrap().join().unwrap();
    }
}

impl InodeCache {
    pub async fn shutdown(self, ring: &Rc<IoUring>) {
        if let Err(e) = self.try_shutdown(ring).await {
            log::error!("Graceful shutdown failed: {}", ErrorFmt(e));
        }
    }

    async fn try_shutdown(self, ring: &Rc<IoUring>) -> Result<(), FuseError> {
        ring.futex_wake(&self.data, i32::MAX, true)
            .map_err(FuseError::FutexWake)?;
        loop {
            let val = self.data.ts.load(Acquire);
            if val == TS_PRUNED || val == TS_EXIT {
                break;
            }
            ring.futex_wait(&self.data, val, true)
                .await
                .map_err(FuseError::FutexWait)?;
        }
        self.data.ts.store(TS_EXIT, Release);
        ring.futex_wake(&self.data, i32::MAX, true)
            .map_err(FuseError::FutexWake)?;
        let _ = ring
            .poll(&self.thread_pipe, 0)
            .await
            .map_err(FuseError::Poll)?;
        Ok(())
    }
}

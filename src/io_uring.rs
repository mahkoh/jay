use {
    crate::{
        async_engine::AsyncEngine,
        io_uring::{
            ops::{async_cancel::AsyncCancelTask, poll::PollTask, write::WriteTask},
            pending_result::PendingResults,
            sys::{
                io_uring_cqe, io_uring_enter, io_uring_params, io_uring_setup, io_uring_sqe,
                IORING_ENTER_GETEVENTS, IORING_FEAT_NODROP, IORING_OFF_CQ_RING, IORING_OFF_SQES,
                IORING_OFF_SQ_RING,
            },
        },
        utils::{
            asyncevent::AsyncEvent,
            bitflags::BitflagsExt,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            mmap::{mmap, Mmapped},
            numcell::NumCell,
            oserror::OsError,
            ptr_ext::{MutPtrExt, PtrExt},
            queue::AsyncQueue,
            stack::Stack,
        },
    },
    std::{
        cell::{Cell, UnsafeCell},
        mem::{self},
        rc::Rc,
        sync::atomic::{
            AtomicU32,
            Ordering::{Acquire, Relaxed, Release},
        },
    },
    thiserror::Error,
    uapi::{
        c::{self},
        OwnedFd,
    },
};

macro_rules! map_err {
    ($n:expr) => {{
        let n = $n;
        if n < 0 {
            Err(crate::utils::oserror::OsError::from(-n as uapi::c::c_int))
        } else {
            Ok(n)
        }
    }};
}
pub use ops::TaskResultExt;

mod ops;
mod pending_result;
mod sys;

#[derive(Debug, Error)]
pub enum IoUringError {
    #[error(transparent)]
    OsError(OsError),
    #[error("Could not create an io-uring")]
    CreateUring(#[source] OsError),
    #[error("The kernel does not support the IORING_FEAT_NODROP feature")]
    NoDrop,
    #[error("Could not map the submission queue ring")]
    MapSqRing(#[source] OsError),
    #[error("Could not map the submission queue entries")]
    MapSqEntries(#[source] OsError),
    #[error("Could not map the completion queue ring")]
    MapCqRing(#[source] OsError),
    #[error("The io-uring has already been destroyed")]
    Destroyed,
    #[error("io_uring_enter failed")]
    Enter(#[source] OsError),
}

pub struct IoUring {
    ring: Rc<IoUringData>,
}

impl Drop for IoUring {
    fn drop(&mut self) {
        self.ring.kill();
    }
}

impl IoUring {
    pub fn new(eng: &Rc<AsyncEngine>, entries: u32) -> Result<Rc<Self>, IoUringError> {
        let mut params = io_uring_params::default();
        let fd = match io_uring_setup(entries, &mut params) {
            Ok(f) => f,
            Err(e) => return Err(IoUringError::CreateUring(e.into())),
        };
        if !params.features.contains(IORING_FEAT_NODROP) {
            return Err(IoUringError::NoDrop);
        }
        let sqmap_map = mmap(
            (params.sq_off.array + params.sq_entries * 4) as _,
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED | c::MAP_POPULATE,
            fd.raw(),
            IORING_OFF_SQ_RING as _,
        );
        let sqmap_map = match sqmap_map {
            Ok(map) => map,
            Err(e) => return Err(IoUringError::MapSqRing(e)),
        };
        let sqesmap_map = mmap(
            params.sq_entries as usize * mem::size_of::<io_uring_sqe>(),
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED | c::MAP_POPULATE,
            fd.raw(),
            IORING_OFF_SQES as _,
        );
        let sqesmap_map = match sqesmap_map {
            Ok(map) => map,
            Err(e) => return Err(IoUringError::MapSqEntries(e)),
        };
        let cqmap_map = mmap(
            params.cq_off.cqes as usize
                + params.sq_entries as usize * mem::size_of::<io_uring_cqe>(),
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED | c::MAP_POPULATE,
            fd.raw(),
            IORING_OFF_CQ_RING as _,
        );
        let cqmap_map = match cqmap_map {
            Ok(map) => map,
            Err(e) => return Err(IoUringError::MapCqRing(e)),
        };
        let sqmask = unsafe {
            *(sqmap_map.ptr as *const u8)
                .add(params.sq_off.ring_mask as _)
                .cast()
        };
        let sqhead = unsafe {
            (sqmap_map.ptr as *const u8)
                .add(params.sq_off.head as _)
                .cast()
        };
        let sqtail = unsafe {
            (sqmap_map.ptr as *const u8)
                .add(params.sq_off.tail as _)
                .cast()
        };
        let sqmap = unsafe {
            let base = (sqmap_map.ptr as *const u8)
                .add(params.sq_off.array as _)
                .cast();
            std::slice::from_raw_parts(base, params.sq_entries as _)
        };
        let sqesmap = unsafe {
            let base = (sqesmap_map.ptr as *const u8).cast();
            std::slice::from_raw_parts(base, params.sq_entries as _)
        };
        let cqmask = unsafe {
            *(cqmap_map.ptr as *const u8)
                .add(params.cq_off.ring_mask as _)
                .cast()
        };
        let cqhead = unsafe {
            (cqmap_map.ptr as *const u8)
                .add(params.cq_off.head as _)
                .cast()
        };
        let cqtail = unsafe {
            (cqmap_map.ptr as *const u8)
                .add(params.cq_off.tail as _)
                .cast()
        };
        let cqmap = unsafe {
            let base = (cqmap_map.ptr as *const u8)
                .add(params.cq_off.cqes as _)
                .cast();
            std::slice::from_raw_parts(base, params.cq_entries as _)
        };
        let data = Rc::new(IoUringData {
            destroyed: Cell::new(false),
            fd,
            eng: eng.clone(),
            _sqesmap_map: sqesmap_map,
            _sqmap_map: sqmap_map,
            sqmask,
            sqlen: params.sq_entries,
            sqhead,
            sqtail,
            sqmap,
            sqesmap,
            _cqmap_map: cqmap_map,
            cqmask,
            cqhead,
            cqtail,
            cqmap,
            cqes_consumed: Default::default(),
            next: Default::default(),
            to_encode: Default::default(),
            pending_in_kernel: Default::default(),
            tasks: Default::default(),
            pending_results: Default::default(),
            cached_writes: Default::default(),
            cached_cancels: Default::default(),
            cached_polls: Default::default(),
        });
        Ok(Rc::new(Self { ring: data }))
    }

    pub fn stop(&self) {
        self.ring.kill();
    }

    pub fn run(&self) -> Result<(), IoUringError> {
        let res = self.ring.run();
        self.ring.kill();
        res
    }
}

struct IoUringData {
    destroyed: Cell<bool>,

    fd: OwnedFd,
    eng: Rc<AsyncEngine>,

    _sqesmap_map: Mmapped,
    _sqmap_map: Mmapped,
    sqmask: u32,
    sqlen: u32,
    sqhead: *const AtomicU32,
    sqtail: *const AtomicU32,
    sqmap: *const [Cell<c::c_uint>],
    sqesmap: *const [UnsafeCell<io_uring_sqe>],

    _cqmap_map: Mmapped,
    cqmask: u32,
    cqhead: *const AtomicU32,
    cqtail: *const AtomicU32,
    cqmap: *const [Cell<io_uring_cqe>],

    cqes_consumed: AsyncEvent,

    next: NumCell<u64>,
    to_encode: AsyncQueue<u64>,
    pending_in_kernel: CopyHashMap<u64, ()>,
    tasks: CopyHashMap<u64, Box<dyn Task>>,

    pending_results: PendingResults,
    cached_writes: Stack<Box<WriteTask>>,
    cached_cancels: Stack<Box<AsyncCancelTask>>,
    cached_polls: Stack<Box<PollTask>>,
}

unsafe trait Task {
    fn id(&self) -> u64;
    fn complete(self: Box<Self>, ring: &IoUringData, res: i32);
    fn encode(&self, sqe: &mut io_uring_sqe);

    fn is_cancel(&self) -> bool {
        false
    }
}

impl IoUringData {
    fn run(&self) -> Result<(), IoUringError> {
        let mut to_submit = 0;
        loop {
            loop {
                self.eng.dispatch();
                if self.destroyed.get() {
                    return Ok(());
                }
                if !self.dispatch_completions() {
                    break;
                }
            }
            to_submit += self.encode();
            let res = if to_submit == 0 {
                io_uring_enter(self.fd.raw(), 0, 1, IORING_ENTER_GETEVENTS)
            } else if self.to_encode.is_empty() {
                io_uring_enter(self.fd.raw(), to_submit as _, 1, IORING_ENTER_GETEVENTS)
            } else {
                io_uring_enter(self.fd.raw(), !0, 0, 0)
            };
            let mut submitted_any = false;
            match res {
                Ok(n) => {
                    if n > 0 {
                        submitted_any = true;
                    }
                    to_submit -= n;
                }
                Err(e) => {
                    if !matches!(e.0, c::EAGAIN | c::EBUSY | c::EINTR) {
                        return Err(IoUringError::Enter(e));
                    }
                }
            }
            if to_submit > 0 && !submitted_any {
                let res = io_uring_enter(self.fd.raw(), 0, 1, IORING_ENTER_GETEVENTS);
                if let Err(e) = res {
                    if e.0 != c::EINTR {
                        return Err(IoUringError::Enter(e));
                    }
                }
            }
        }
    }

    fn dispatch_completions(&self) -> bool {
        unsafe {
            let mut head = self.cqhead.deref().load(Relaxed);
            let tail = self.cqtail.deref().load(Acquire);
            if head == tail {
                return false;
            }
            while head != tail {
                let idx = (head & self.cqmask) as usize;
                let entry = self.cqmap.deref()[idx].get();
                head = head.wrapping_add(1);
                self.cqhead.deref().store(head, Release);
                if let Some(pending) = self.tasks.remove(&entry.user_data) {
                    self.pending_in_kernel.remove(&entry.user_data);
                    pending.complete(&self, entry.res);
                }
            }
            self.cqhead.deref().store(head, Release);
            self.cqes_consumed.trigger();
            true
        }
    }

    fn encode(&self) -> usize {
        let tasks = self.tasks.lock();
        let mut encoded = 0;
        unsafe {
            let mut tail = self.sqtail.deref().load(Relaxed);
            let head = self.sqhead.deref().load(Acquire);
            while tail.wrapping_sub(head) < self.sqlen {
                let id = match self.to_encode.try_pop() {
                    Some(t) => t,
                    _ => break,
                };
                let task = match tasks.get(&id) {
                    Some(t) => t,
                    _ => continue,
                };
                self.pending_in_kernel.set(id, ());
                let idx = (tail & self.sqmask) as usize;
                let mut sqe = self.sqesmap.deref()[idx].get().deref_mut();
                self.sqmap.deref()[idx].set(idx as _);
                *sqe = Default::default();
                sqe.user_data = id;
                task.encode(sqe);
                tail = tail.wrapping_add(1);
                encoded += 1;
            }
            self.sqtail.deref().store(tail, Release);
        }
        encoded
    }

    fn id(&self) -> Cancellable {
        Cancellable {
            id: self.id_raw(),
            data: self,
        }
    }

    fn id_raw(&self) -> u64 {
        self.next.fetch_add(1)
    }

    fn cancel_task(&self, id: u64) {
        if !self.tasks.contains(&id) {
            return;
        }
        if !self.pending_in_kernel.contains(&id) {
            self.tasks
                .remove(&id)
                .unwrap()
                .complete(self, -c::ECANCELED);
            return;
        }
        self.cancel_task_in_kernel(id);
    }

    fn schedule(&self, t: Box<dyn Task>) {
        assert!(!self.destroyed.get());
        self.to_encode.push(t.id());
        self.tasks.set(t.id(), t);
    }

    fn check_destroyed(&self) -> Result<(), IoUringError> {
        if self.destroyed.get() {
            Err(IoUringError::Destroyed)
        } else {
            Ok(())
        }
    }

    fn kill(&self) {
        let mut to_cancel = vec![];
        for task in self.tasks.lock().values() {
            if !task.is_cancel() {
                to_cancel.push(task.id());
            }
        }
        for task in to_cancel {
            self.cancel_task(task);
        }
        self.destroyed.set(true);
        while !self.tasks.is_empty() {
            self.encode();
            let _ = io_uring_enter(self.fd.raw(), u32::MAX, 0, 0);
            let res = io_uring_enter(self.fd.raw(), 0, 1, IORING_ENTER_GETEVENTS);
            if let Err(e) = res {
                panic!("Could not wait for io_uring to drain: {}", ErrorFmt(e));
            }
            while self.dispatch_completions() {
                // nothing
            }
        }
    }
}

struct Cancellable<'a> {
    id: u64,
    data: &'a IoUringData,
}

impl<'a> Drop for Cancellable<'a> {
    fn drop(&mut self) {
        self.data.cancel_task(self.id);
    }
}

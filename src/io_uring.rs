pub use ops::{
    TaskResultExt,
    poll_external::{PendingPoll, PollCallback},
    timeout_external::{PendingTimeout, TimeoutCallback},
};
use {
    crate::{
        async_engine::AsyncEngine,
        io_uring::{
            ops::{
                accept::AcceptTask, async_cancel::AsyncCancelTask, connect::ConnectTask,
                poll::PollTask, poll_external::PollExternalTask, read_write::ReadWriteTask,
                read_write_no_cancel::ReadWriteNoCancelTask, recvmsg::RecvmsgTask,
                sendmsg::SendmsgTask, timeout::TimeoutTask, timeout_external::TimeoutExternalTask,
                timeout_link::TimeoutLinkTask,
            },
            pending_result::PendingResults,
            sys::{
                IORING_ENTER_GETEVENTS, IORING_FEAT_NODROP, IORING_OFF_CQ_RING, IORING_OFF_SQ_RING,
                IORING_OFF_SQES, IOSQE_IO_LINK, io_uring_cqe, io_uring_enter, io_uring_params,
                io_uring_setup, io_uring_sqe,
            },
        },
        utils::{
            asyncevent::AsyncEvent,
            bitflags::BitflagsExt,
            buf::Buf,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            mmap::{Mmapped, mmap},
            oserror::OsError,
            ptr_ext::{MutPtrExt, PtrExt},
            stack::Stack,
            syncqueue::SyncQueue,
        },
    },
    std::{
        cell::{Cell, RefCell, UnsafeCell},
        rc::Rc,
        sync::atomic::{
            AtomicU32,
            Ordering::{Acquire, Relaxed, Release},
        },
    },
    thiserror::Error,
    uapi::{
        OwnedFd,
        c::{self},
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

mod ops;
mod pending_result;
mod sys;

#[derive(Debug, Error)]
pub enum IoUringError {
    #[error(transparent)]
    OsError(#[from] OsError),
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
    #[error("Kernel sent invalid cmsg data")]
    InvalidCmsgData,
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
            Err(e) => return Err(IoUringError::CreateUring(e)),
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
            params.sq_entries as usize * size_of::<io_uring_sqe>(),
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
            params.cq_off.cqes as usize + params.cq_entries as usize * size_of::<io_uring_cqe>(),
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
            cached_read_writes: Default::default(),
            cached_read_writes_no_cancel: Default::default(),
            cached_cancels: Default::default(),
            cached_polls: Default::default(),
            cached_polls_external: Default::default(),
            cached_sendmsg: Default::default(),
            cached_recvmsg: Default::default(),
            cached_timeouts: Default::default(),
            cached_timeouts_external: Default::default(),
            cached_timeout_links: Default::default(),
            cached_cmsg_bufs: Default::default(),
            cached_connects: Default::default(),
            cached_accepts: Default::default(),
            fd_ids_scratch: Default::default(),
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

    pub fn cancel(&self, id: IoUringTaskId) {
        self.ring.cancel_task(id);
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

    next: IoUringTaskIds,
    to_encode: SyncQueue<IoUringTaskId>,
    pending_in_kernel: CopyHashMap<IoUringTaskId, ()>,
    tasks: CopyHashMap<IoUringTaskId, Box<dyn Task>>,

    pending_results: PendingResults,

    cached_read_writes: Stack<Box<ReadWriteTask>>,
    cached_read_writes_no_cancel: Stack<Box<ReadWriteNoCancelTask>>,
    cached_cancels: Stack<Box<AsyncCancelTask>>,
    cached_polls: Stack<Box<PollTask>>,
    cached_polls_external: Stack<Box<PollExternalTask>>,
    cached_sendmsg: Stack<Box<SendmsgTask>>,
    cached_recvmsg: Stack<Box<RecvmsgTask>>,
    cached_timeouts: Stack<Box<TimeoutTask>>,
    cached_timeouts_external: Stack<Box<TimeoutExternalTask>>,
    cached_timeout_links: Stack<Box<TimeoutLinkTask>>,
    cached_cmsg_bufs: Stack<Buf>,
    cached_connects: Stack<Box<ConnectTask>>,
    cached_accepts: Stack<Box<AcceptTask>>,

    fd_ids_scratch: RefCell<Vec<c::c_int>>,
}

unsafe trait Task {
    fn id(&self) -> IoUringTaskId;
    fn complete(self: Box<Self>, ring: &IoUringData, res: i32);
    fn encode(&self, sqe: &mut io_uring_sqe);

    fn is_cancel(&self) -> bool {
        false
    }

    fn has_timeout(&self) -> bool {
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
                let id = IoUringTaskId(entry.user_data);
                if let Some(pending) = self.tasks.remove(&id) {
                    self.pending_in_kernel.remove(&id);
                    pending.complete(self, entry.res);
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
            let available = self.sqlen - tail.wrapping_sub(head);
            while encoded < available {
                let id = match self.to_encode.pop() {
                    Some(t) => t,
                    _ => break,
                };
                let task = match tasks.get(&id) {
                    Some(t) => t,
                    _ => continue,
                };
                let has_timeout = task.has_timeout();
                if has_timeout && (available - encoded) < 2 {
                    self.to_encode.push_front(id);
                    break;
                }
                self.pending_in_kernel.set(id, ());
                let idx = (tail & self.sqmask) as usize;
                let sqe = self.sqesmap.deref()[idx].get().deref_mut();
                self.sqmap.deref()[idx].set(idx as _);
                *sqe = Default::default();
                sqe.user_data = id.raw();
                task.encode(sqe);
                if has_timeout {
                    sqe.flags |= IOSQE_IO_LINK;
                }
                tail = tail.wrapping_add(1);
                encoded += 1;
            }
            self.sqtail.deref().store(tail, Release);
        }
        encoded as usize
    }

    fn id(&self) -> Cancellable {
        Cancellable {
            id: self.id_raw(),
            data: self,
        }
    }

    fn id_raw(&self) -> IoUringTaskId {
        self.next.next()
    }

    fn cancel_task(&self, id: IoUringTaskId) {
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
        self.eng.stop();
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

    fn cmsg_buf(&self) -> Buf {
        self.cached_cmsg_bufs.pop().unwrap_or_else(|| Buf::new(256))
    }
}

linear_ids!(IoUringTaskIds, IoUringTaskId, u64);

#[expect(clippy::derivable_impls)]
impl Default for IoUringTaskId {
    fn default() -> Self {
        Self(0)
    }
}

struct Cancellable<'a> {
    id: IoUringTaskId,
    data: &'a IoUringData,
}

impl<'a> Drop for Cancellable<'a> {
    fn drop(&mut self) {
        self.data.cancel_task(self.id);
    }
}

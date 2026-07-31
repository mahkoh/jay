use crate::io_uring::IoUring;
use crate::io_uring::IoUringData;
use crate::io_uring::IoUringError;
use crate::io_uring::IoUringTaskId;
use crate::io_uring::Task;
use crate::io_uring::TaskResultExt;
use crate::io_uring::pending_result::PendingResult;
use crate::io_uring::sys::IORING_OP_FUTEX_WAIT;
use crate::io_uring::sys::IORING_OP_FUTEX_WAKE;
use crate::io_uring::sys::io_uring_sqe;
use std::rc::Rc;
use std::sync::atomic::AtomicU32;

pub trait FutexObj: 'static {
    fn get(&self) -> &AtomicU32;
}

#[expect(dead_code)]
const FUTEX2_SIZE_U8: i32 = 0x00;
#[expect(dead_code)]
const FUTEX2_SIZE_U16: i32 = 0x01;
const FUTEX2_SIZE_U32: i32 = 0x02;
#[expect(dead_code)]
const FUTEX2_SIZE_U64: i32 = 0x03;
#[expect(dead_code)]
const FUTEX2_NUMA: i32 = 0x04;
#[expect(dead_code)]
const FUTEX2_MPOL: i32 = 0x08;
const FUTEX2_PRIVATE: i32 = 128;

impl IoUring {
    #[expect(dead_code)]
    pub fn futex_wake(
        &self,
        futex: &Rc<impl FutexObj>,
        n: i32,
        private: bool,
    ) -> Result<(), IoUringError> {
        self.ring.check_destroyed()?;
        {
            let mut pw = self.ring.cached_futex_wakes.pop().unwrap_or_default();
            pw.id = self.ring.id_raw();
            pw.addr = futex.get().as_ptr();
            pw.n = n;
            pw.private = private;
            pw.futex = Some(futex.clone());
            self.ring.schedule(pw);
        }
        Ok(())
    }

    #[expect(dead_code)]
    pub async fn futex_wait(
        &self,
        futex: &Rc<impl FutexObj>,
        val: u32,
        private: bool,
    ) -> Result<(), IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_futex_waits.pop().unwrap_or_default();
            pw.id = id.id;
            pw.addr = futex.get().as_ptr();
            pw.val = val;
            pw.private = private;
            pw.data = Some(FutexWaitData {
                pr: pr.clone(),
                _futex: futex.clone(),
            });
            self.ring.schedule(pw);
        }
        Ok(pr.await.map(drop)).merge()
    }
}

#[derive(Default)]
pub struct FutexWakeTask {
    id: IoUringTaskId,
    addr: *mut u32,
    n: i32,
    private: bool,
    futex: Option<Rc<dyn FutexObj>>,
}

struct FutexWaitData {
    pr: PendingResult,
    _futex: Rc<dyn FutexObj>,
}

#[derive(Default)]
pub struct FutexWaitTask {
    id: IoUringTaskId,
    addr: *mut u32,
    val: u32,
    private: bool,
    data: Option<FutexWaitData>,
}

unsafe impl Task for FutexWakeTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, _res: i32) {
        self.futex.take();
        ring.cached_futex_wakes.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        encode_futex(
            sqe,
            IORING_OP_FUTEX_WAKE,
            self.addr,
            self.n as _,
            self.private,
        );
    }
}

unsafe impl Task for FutexWaitTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.pr.complete(res);
        }
        ring.cached_futex_waits.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        encode_futex(
            sqe,
            IORING_OP_FUTEX_WAIT,
            self.addr,
            self.val as _,
            self.private,
        );
    }
}

fn encode_futex(sqe: &mut io_uring_sqe, op: u8, addr: *mut u32, val: u64, private: bool) {
    let mut flags = FUTEX2_SIZE_U32;
    if private {
        flags |= FUTEX2_PRIVATE;
    }
    sqe.opcode = op;
    sqe.fd = flags;
    sqe.u2.addr = addr as _;
    sqe.u1.addr2 = val as _;
    sqe.u6.s1.addr3 = !0u32 as u64;
}

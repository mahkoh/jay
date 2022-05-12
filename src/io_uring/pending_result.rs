use {
    crate::utils::{numcell::NumCell, oserror::OsError, ptr_ext::PtrExt, stack::Stack},
    std::{
        cell::Cell,
        future::Future,
        pin::Pin,
        rc::{Rc, Weak},
        task::{Context, Poll, Waker},
    },
    uapi::c,
};

#[derive(Default)]
pub struct PendingResults {
    data: Rc<PendingResultsData>,
}

impl PendingResults {
    pub fn acquire(&self) -> PendingResult {
        let pr = self.data.unused.pop().unwrap_or_else(|| {
            Box::into_raw(Box::new(PendingResultData {
                rc: NumCell::new(0),
                base: Rc::downgrade(&self.data),
                waker: Cell::new(None),
                res: Cell::new(None),
            }))
        });
        unsafe {
            let prr = pr.deref();
            debug_assert_eq!(prr.rc.get(), 0);
            prr.rc.fetch_add(1);
            PendingResult { pr }
        }
    }
}

#[derive(Default)]
struct PendingResultsData {
    unused: Stack<*mut PendingResultData>,
}

impl Drop for PendingResultsData {
    fn drop(&mut self) {
        while let Some(pr) = self.unused.pop() {
            unsafe {
                drop(Box::from_raw(pr));
            }
        }
    }
}

struct PendingResultData {
    rc: NumCell<u32>,
    base: Weak<PendingResultsData>,
    waker: Cell<Option<Waker>>,
    res: Cell<Option<i32>>,
}

pub struct PendingResult {
    pr: *mut PendingResultData,
}

impl PendingResult {
    pub fn complete(&self, res: i32) {
        unsafe {
            let pr = self.pr.deref();
            pr.res.set(Some(res));
            if let Some(waker) = pr.waker.take() {
                waker.wake();
            }
        }
    }
}

impl Drop for PendingResult {
    fn drop(&mut self) {
        {
            let pr = unsafe { self.pr.deref() };
            if pr.rc.fetch_sub(1) != 1 {
                return;
            }
            if let Some(base) = pr.base.upgrade() {
                pr.waker.set(None);
                pr.res.set(None);
                base.unused.push(self.pr);
                return;
            }
        }
        unsafe {
            drop(Box::from_raw(self.pr));
        }
    }
}

impl Clone for PendingResult {
    fn clone(&self) -> Self {
        let pr = unsafe { self.pr.deref() };
        pr.rc.fetch_add(1);
        Self { pr: self.pr }
    }
}

impl Future for PendingResult {
    type Output = Result<i32, OsError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pr = unsafe { self.pr.deref() };
        if let Some(res) = pr.res.take() {
            let res = if res < 0 {
                Err(OsError::from(-res as c::c_int))
            } else {
                Ok(res)
            };
            Poll::Ready(res)
        } else {
            pr.waker.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}

use {
    crate::{
        async_engine::{ae_queue::DispatchQueue, Phase},
        utils::{
            numcell::NumCell,
            ptr_ext::{MutPtrExt, PtrExt},
        },
    },
    std::{
        cell::{Cell, UnsafeCell},
        future::Future,
        mem::ManuallyDrop,
        pin::Pin,
        ptr,
        rc::Rc,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    },
};

#[must_use]
pub struct SpawnedFuture<T: 'static> {
    vtable: &'static SpawnedFutureVtable<T>,
    data: *mut u8,
}

impl<T> Future for SpawnedFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { (self.vtable.poll)(self.data, cx) }
    }
}

impl<T> Drop for SpawnedFuture<T> {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(self.data);
        }
    }
}

struct SpawnedFutureVTableProxy<T, F>(T, F);

impl<T: 'static, F: Future<Output = T>> SpawnedFutureVTableProxy<T, F> {
    const VTABLE: &'static SpawnedFutureVtable<T> = &SpawnedFutureVtable {
        poll: Self::poll,
        drop: Self::drop,
    };

    unsafe fn poll(data: *mut u8, ctx: &mut Context<'_>) -> Poll<T> {
        let task = (data as *const Task<T, F>).deref();
        if &task.state & COMPLETED == 0 {
            task.waker.set(Some(ctx.waker().clone()));
            Poll::Pending
        } else if &task.state & EMPTIED == 0 {
            task.state.or_assign(EMPTIED);
            Poll::Ready(ptr::read(&*task.data.get().deref().result))
        } else {
            panic!("Future polled after it has already been emptied");
        }
    }

    unsafe fn drop(data: *mut u8) {
        {
            let task = (data as *const Task<T, F>).deref();
            task.state.or_assign(CANCELLED);
            if &task.state & RUNNING == 0 {
                task.drop_data();
            }
        }
        Task::<T, F>::dec_ref_count(data as _);
    }
}

struct SpawnedFutureVtable<T> {
    poll: unsafe fn(data: *mut u8, ctx: &mut Context<'_>) -> Poll<T>,
    drop: unsafe fn(data: *mut u8),
}

union TaskData<T, F: Future<Output = T>> {
    result: ManuallyDrop<T>,
    future: ManuallyDrop<F>,
}

const RUNNING: u32 = 1;
const RUN_AGAIN: u32 = 2;
const COMPLETED: u32 = 4;
const EMPTIED: u32 = 8;
const CANCELLED: u32 = 16;

struct Task<T, F: Future<Output = T>> {
    ref_count: NumCell<u64>,
    phase: Phase,
    state: NumCell<u32>,
    data: UnsafeCell<TaskData<T, F>>,
    waker: Cell<Option<Waker>>,
    queue: Rc<DispatchQueue>,
}

pub(super) struct Runnable {
    data: *const u8,
    run: unsafe fn(data: *const u8, run: bool),
}

impl Runnable {
    pub(super) fn run(self) {
        let slf = ManuallyDrop::new(self);
        unsafe {
            (slf.run)(slf.data, true);
        }
    }
}

impl Drop for Runnable {
    fn drop(&mut self) {
        unsafe {
            (self.run)(self.data, false);
        }
    }
}

impl DispatchQueue {
    pub(super) fn spawn<T, F: Future<Output = T>>(
        self: &Rc<Self>,
        phase: Phase,
        f: F,
    ) -> SpawnedFuture<T> {
        let f = Box::new(Task {
            ref_count: NumCell::new(1),
            phase,
            state: NumCell::new(0),
            data: UnsafeCell::new(TaskData {
                future: ManuallyDrop::new(f),
            }),
            waker: Cell::new(None),
            queue: self.clone(),
        });
        unsafe {
            f.schedule_run();
        }
        let f = Box::into_raw(f);
        SpawnedFuture {
            vtable: SpawnedFutureVTableProxy::<T, F>::VTABLE,
            data: f as _,
        }
    }
}

impl<T, F: Future<Output = T>> Task<T, F> {
    const VTABLE: &'static RawWakerVTable = &RawWakerVTable::new(
        Self::waker_clone,
        Self::waker_wake,
        Self::waker_wake_by_ref,
        Self::waker_drop,
    );

    unsafe fn run_proxy(data: *const u8, run: bool) {
        let task = data as *const Self;
        if run {
            task.deref().run();
        }
        Self::dec_ref_count(task);
    }

    unsafe fn dec_ref_count(slf: *const Self) {
        if slf.deref().ref_count.fetch_sub(1) == 1 {
            Box::from_raw(slf as *mut Self);
        }
    }

    unsafe fn inc_ref_count(&self) {
        self.ref_count.fetch_add(1);
    }

    unsafe fn waker_clone(data: *const ()) -> RawWaker {
        let task = &mut *(data as *mut Self);
        task.inc_ref_count();
        RawWaker::new(data, Self::VTABLE)
    }

    unsafe fn waker_wake(data: *const ()) {
        Self::waker_wake_by_ref(data);
        Self::waker_drop(data);
    }

    unsafe fn waker_wake_by_ref(data: *const ()) {
        (data as *const Self).deref().schedule_run();
    }

    unsafe fn waker_drop(data: *const ()) {
        Self::dec_ref_count(data as _)
    }

    unsafe fn schedule_run(&self) {
        if &self.state & (COMPLETED | CANCELLED) == 0 {
            if &self.state & RUNNING == 0 {
                self.state.or_assign(RUNNING);
                self.inc_ref_count();
                let data = self as *const _ as _;
                self.queue.push(
                    Runnable {
                        data,
                        run: Self::run_proxy,
                    },
                    self.phase,
                );
            } else {
                self.state.or_assign(RUN_AGAIN);
            }
        }
    }

    unsafe fn run(&self) {
        if &self.state & CANCELLED == 0 {
            let data = self.data.get().deref_mut();
            self.inc_ref_count();
            let raw_waker = RawWaker::new(self as *const _ as _, Self::VTABLE);
            let waker = Waker::from_raw(raw_waker);

            let mut ctx = Context::from_waker(&waker);
            if let Poll::Ready(d) = Pin::new_unchecked(&mut *data.future).poll(&mut ctx) {
                ManuallyDrop::drop(&mut data.future);
                ptr::write(&mut data.result, ManuallyDrop::new(d));
                self.state.or_assign(COMPLETED);
                if let Some(waker) = self.waker.take() {
                    waker.wake();
                }
            }
        }

        self.state.and_assign(!RUNNING);

        if &self.state & CANCELLED != 0 {
            self.drop_data();
        } else if &self.state & RUN_AGAIN != 0 {
            self.state.and_assign(!RUN_AGAIN);
            self.schedule_run()
        }
    }

    unsafe fn drop_data(&self) {
        if &self.state & COMPLETED == 0 {
            ManuallyDrop::drop(&mut self.data.get().deref_mut().future);
        } else if &self.state & EMPTIED == 0 {
            ManuallyDrop::drop(&mut self.data.get().deref_mut().result);
        }
    }
}

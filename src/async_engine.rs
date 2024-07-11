mod ae_task;
mod ae_yield;

pub use {crate::async_engine::ae_yield::Yield, ae_task::SpawnedFuture};
use {
    crate::{
        async_engine::ae_task::Runnable,
        time::Time,
        utils::{array, numcell::NumCell, syncqueue::SyncQueue},
    },
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        future::Future,
        rc::Rc,
        task::Waker,
    },
};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Phase {
    EventHandling,
    Layout,
    PostLayout,
    Present,
}
const NUM_PHASES: usize = 4;

pub struct AsyncEngine {
    num_queued: NumCell<usize>,
    queues: [SyncQueue<Runnable>; NUM_PHASES],
    iteration: NumCell<u64>,
    yields: SyncQueue<Waker>,
    stash: RefCell<VecDeque<Runnable>>,
    yield_stash: RefCell<VecDeque<Waker>>,
    stopped: Cell<bool>,
    now: Cell<Option<Time>>,
}

impl AsyncEngine {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            num_queued: Default::default(),
            queues: array::from_fn(|_| Default::default()),
            iteration: Default::default(),
            yields: Default::default(),
            stash: Default::default(),
            yield_stash: Default::default(),
            stopped: Cell::new(false),
            now: Default::default(),
        })
    }

    pub fn stop(&self) {
        self.stopped.set(true);
    }

    pub fn clear(&self) {
        self.stash.borrow_mut().clear();
        self.yield_stash.borrow_mut().clear();
        self.yields.take();
        for queue in &self.queues {
            queue.take();
        }
    }

    pub fn spawn<T, F: Future<Output = T> + 'static>(self: &Rc<Self>, f: F) -> SpawnedFuture<T> {
        self.spawn_(Phase::EventHandling, f)
    }

    pub fn spawn2<T, F: Future<Output = T> + 'static>(
        self: &Rc<Self>,
        phase: Phase,
        f: F,
    ) -> SpawnedFuture<T> {
        self.spawn_(phase, f)
    }

    pub fn yield_now(self: &Rc<Self>) -> Yield {
        Yield {
            iteration: self.iteration(),
            queue: self.clone(),
        }
    }

    pub fn dispatch(&self) {
        let mut stash = self.stash.borrow_mut();
        let mut yield_stash = self.yield_stash.borrow_mut();
        while self.num_queued.get() > 0 {
            self.now.take();
            self.iteration.fetch_add(1);
            let mut phase = 0;
            while phase < NUM_PHASES {
                self.queues[phase].swap(&mut *stash);
                if stash.is_empty() {
                    phase += 1;
                    continue;
                }
                self.num_queued.fetch_sub(stash.len());
                for runnable in stash.drain(..) {
                    runnable.run();
                    if self.stopped.get() {
                        return;
                    }
                }
            }
            self.yields.swap(&mut *yield_stash);
            for waker in yield_stash.drain(..) {
                waker.wake();
            }
        }
    }

    fn push(&self, runnable: Runnable, phase: Phase) {
        self.queues[phase as usize].push(runnable);
        self.num_queued.fetch_add(1);
    }

    fn push_yield(&self, waker: Waker) {
        self.yields.push(waker);
    }

    fn iteration(&self) -> u64 {
        self.iteration.get()
    }

    pub fn now(&self) -> Time {
        match self.now.get() {
            Some(t) => t,
            None => {
                let now = Time::now_unchecked();
                self.now.set(Some(now));
                now
            }
        }
    }
}

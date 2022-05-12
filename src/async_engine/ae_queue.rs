use {
    crate::{
        async_engine::{ae_task::Runnable, AsyncError, Phase, NUM_PHASES},
        event_loop::{EventLoop, EventLoopDispatcher, EventLoopId},
        utils::{array, numcell::NumCell, syncqueue::SyncQueue},
    },
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        error::Error,
        rc::Rc,
        task::Waker,
    },
};

pub(super) struct Dispatcher {
    queue: Rc<DispatchQueue>,
    stash: RefCell<VecDeque<Runnable>>,
    yield_stash: RefCell<VecDeque<Waker>>,
}

impl Dispatcher {
    pub fn install(el: &Rc<EventLoop>) -> Result<Rc<DispatchQueue>, AsyncError> {
        let id = el.id();
        let queue = Rc::new(DispatchQueue {
            id,
            el: el.clone(),
            dispatch_scheduled: Cell::new(false),
            num_queued: Default::default(),
            queues: array::from_fn(|_| Default::default()),
            iteration: Default::default(),
            yields: Default::default(),
        });
        let slf = Rc::new(Dispatcher {
            queue: queue.clone(),
            stash: Default::default(),
            yield_stash: Default::default(),
        });
        el.insert(id, None, 0, slf)?;
        Ok(queue)
    }
}

impl EventLoopDispatcher for Dispatcher {
    fn dispatch(self: Rc<Self>, _fd: Option<i32>, _events: i32) -> Result<(), Box<dyn Error>> {
        let mut stash = self.stash.borrow_mut();
        let mut yield_stash = self.yield_stash.borrow_mut();
        while self.queue.num_queued.get() > 0 {
            self.queue.iteration.fetch_add(1);
            let mut phase = 0;
            while phase < NUM_PHASES as usize {
                self.queue.queues[phase].swap(&mut *stash);
                if stash.is_empty() {
                    phase += 1;
                    continue;
                }
                self.queue.num_queued.fetch_sub(stash.len());
                for runnable in stash.drain(..) {
                    runnable.run();
                }
            }
            self.queue.yields.swap(&mut *yield_stash);
            for waker in yield_stash.drain(..) {
                waker.wake();
            }
        }
        self.queue.dispatch_scheduled.set(false);
        Ok(())
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        let _ = self.queue.el.remove(self.queue.id);
        for queue in &self.queue.queues {
            queue.swap(&mut VecDeque::new());
        }
    }
}

pub(super) struct DispatchQueue {
    dispatch_scheduled: Cell<bool>,
    id: EventLoopId,
    el: Rc<EventLoop>,
    num_queued: NumCell<usize>,
    queues: [SyncQueue<Runnable>; NUM_PHASES],
    iteration: NumCell<u64>,
    yields: SyncQueue<Waker>,
}

impl DispatchQueue {
    pub fn clear(&self) {
        self.yields.take();
        for queue in &self.queues {
            queue.take();
        }
    }

    pub fn push(&self, runnable: Runnable, phase: Phase) {
        self.queues[phase as usize].push(runnable);
        self.num_queued.fetch_add(1);
        if !self.dispatch_scheduled.get() {
            let _ = self.el.schedule(self.id);
            self.dispatch_scheduled.set(true);
        }
    }

    pub fn push_yield(&self, waker: Waker) {
        self.yields.push(waker);
    }

    pub fn iteration(&self) -> u64 {
        self.iteration.get()
    }
}

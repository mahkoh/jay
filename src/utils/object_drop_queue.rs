use {
    crate::{
        io_uring::{IoUring, PendingPoll, PollCallback},
        utils::{errorfmt::ErrorFmt, oserror::OsError, stack::Stack},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    uapi::{OwnedFd, c::c_short},
};

pub struct ObjectDropQueue<T> {
    ring: Rc<IoUring>,
    killed: Cell<bool>,
    pending: RefCell<Vec<Option<(T, PendingPoll)>>>,
    stack: Stack<Rc<Pollable<T>>>,
}

struct Pollable<T> {
    queue: Rc<ObjectDropQueue<T>>,
    idx: usize,
}

impl<T> ObjectDropQueue<T> {
    pub fn new(ring: &Rc<IoUring>) -> Self {
        Self {
            ring: ring.clone(),
            killed: Default::default(),
            pending: Default::default(),
            stack: Default::default(),
        }
    }

    #[expect(dead_code)]
    pub fn push(self: &Rc<Self>, fd: &Rc<OwnedFd>, t: T)
    where
        T: 'static,
    {
        if self.killed.get() {
            return;
        }
        let pending = &mut *self.pending.borrow_mut();
        let pollable = match self.stack.pop() {
            Some(p) => p,
            None => {
                let pollable = Rc::new(Pollable {
                    queue: self.clone(),
                    idx: pending.len(),
                });
                pending.push(None);
                pollable
            }
        };
        let idx = pollable.idx;
        match self.ring.readable_external(fd, pollable) {
            Ok(p) => {
                pending[idx] = Some((t, p));
            }
            Err(e) => {
                log::error!("Could not register object: {}", ErrorFmt(e));
            }
        }
    }

    pub fn kill(&self) {
        self.killed.set(true);
        self.pending.take();
        self.stack.take();
    }
}

impl<T> PollCallback for Pollable<T> {
    fn completed(self: Rc<Self>, res: Result<c_short, OsError>) {
        if let Err(e) = res {
            log::error!("Could not wait for fd to become readable: {}", ErrorFmt(e));
        }
        let q = &self.queue;
        if !q.killed.get() {
            q.pending.borrow_mut()[self.idx] = None;
            q.stack.push(self.clone());
        }
    }
}

use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

#[cfg(test)]
mod tests;

#[derive(Copy, Clone)]
pub struct ThreadId {
    id: u64,
}

thread_local! {
    static THREAD_ID: ThreadId = {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = ThreadId {
            id: NEXT.fetch_add(1, Relaxed),
        };
        THREAD_ID_ID.set(id.id);
        id
    };
    static THREAD_ID_ID: Cell<u64> = const { Cell::new(0) };
}

impl Default for ThreadId {
    fn default() -> Self {
        ThreadId::current()
    }
}

impl ThreadId {
    pub fn current() -> Self {
        THREAD_ID.with(|id| *id)
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        self.id == THREAD_ID_ID.get()
    }

    #[cfg_attr(not(test), expect(dead_code))]
    #[inline]
    pub fn is_not_current(&self) -> bool {
        !self.is_current()
    }
}

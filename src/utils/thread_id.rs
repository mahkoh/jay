use std::{cell::Cell, ptr, sync::Arc};

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct ThreadId {
    id: Arc<usize>,
}

thread_local! {
    static THREAD_ID: ThreadId = {
        let id = ThreadId {
            id: Arc::new(0),
        };
        THREAD_ID_ADDR.set(id.addr());
        id
    };
    static THREAD_ID_ADDR: Cell<*const usize> = const { Cell::new(ptr::null()) };
}

impl Default for ThreadId {
    fn default() -> Self {
        ThreadId::current()
    }
}

impl ThreadId {
    pub fn current() -> Self {
        THREAD_ID.with(|tid| tid.clone())
    }

    #[inline]
    fn addr(&self) -> *const usize {
        let reference: &usize = &self.id;
        reference as *const usize
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        self.addr() == THREAD_ID_ADDR.get()
    }

    #[cfg_attr(not(test), expect(dead_code))]
    #[inline]
    pub fn is_not_current(&self) -> bool {
        !self.is_current()
    }
}

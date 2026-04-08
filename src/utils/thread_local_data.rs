use {
    crate::utils::{stack::Stack, thread_id::ThreadId},
    isnt::std_1::primitive::IsntConstPtrExt,
    std::{any::Any, cell::Cell, convert::Infallible, rc::Rc},
};

#[cfg(test)]
mod tests;

pub struct ThreadLocalData<T>
where
    T: 'static,
{
    thread_id: ThreadId,
    ptr: Cell<*const T>,
}

thread_local! {
    static HOLDER: Stack<Rc<dyn Any>> = const { Stack::new() };
}

unsafe impl<T> Send for ThreadLocalData<T> {}
unsafe impl<T> Sync for ThreadLocalData<T> {}

impl<T> ThreadLocalData<T>
where
    T: 'static,
{
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new(thread_id: &ThreadId) -> Self {
        Self {
            thread_id: *thread_id,
            ptr: Default::default(),
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn get_or_create(&self, init: impl FnOnce() -> Rc<T>) -> Rc<T> {
        self.get_or_try_create::<Infallible>(|| Ok(init())).unwrap()
    }

    pub fn get_or_try_create<E>(
        &self,
        init: impl FnOnce() -> Result<Rc<T>, E>,
    ) -> Result<Rc<T>, E> {
        assert!(self.thread_id.is_current());
        HOLDER.with(|h| {
            let ptr = self.ptr.get();
            if ptr.is_not_null() {
                unsafe {
                    Rc::increment_strong_count(ptr);
                    return Ok(Rc::from_raw(ptr));
                }
            }
            let rc = init()?;
            h.push(rc.clone());
            self.ptr.set(Rc::as_ptr(&rc));
            Ok(rc)
        })
    }
}

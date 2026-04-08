use {
    crate::utils::thread_id::ThreadId,
    std::{
        fmt::{Debug, Formatter},
        ops::Deref,
        rc::Rc,
    },
};

pub struct SendSyncRc<T> {
    tid: ThreadId,
    v: Rc<T>,
}

impl<T> SendSyncRc<T> {
    #[expect(dead_code)]
    pub fn new(current: &ThreadId, v: &Rc<T>) -> Self {
        assert!(current.is_current());
        Self {
            tid: *current,
            v: v.clone(),
        }
    }
}

impl<T> Drop for SendSyncRc<T> {
    fn drop(&mut self) {
        assert!(self.tid.is_current());
    }
}

impl<T> Deref for SendSyncRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}

unsafe impl<T> Send for SendSyncRc<T> where T: Sync {}
unsafe impl<T> Sync for SendSyncRc<T> where T: Sync {}

impl<T> Debug for SendSyncRc<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}

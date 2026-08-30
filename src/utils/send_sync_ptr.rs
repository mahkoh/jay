#[expect(unused)]
pub struct SendSyncPtrMut<T>(pub *mut T);

unsafe impl<T> Send for SendSyncPtrMut<T> {}
unsafe impl<T> Sync for SendSyncPtrMut<T> {}

pub struct SendSyncPtrConst<T>(pub *const T);

unsafe impl<T> Send for SendSyncPtrConst<T> {}
unsafe impl<T> Sync for SendSyncPtrConst<T> {}

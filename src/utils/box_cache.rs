use {
    crate::utils::{box_ext::BoxExt, reset::Reset, stack::Stack},
    derivative::Derivative,
    std::{
        mem,
        mem::{ManuallyDrop, MaybeUninit},
        ops::{Deref, DerefMut},
        rc::Rc,
    },
};

pub trait BoxCacheMethod<T> {
    type Cached;

    fn into_cached(boxed: Box<T>) -> Box<Self::Cached>;
}

pub struct BoxUninit;

impl<T> BoxCacheMethod<T> for BoxUninit {
    type Cached = MaybeUninit<T>;

    fn into_cached(boxed: Box<T>) -> Box<Self::Cached> {
        Box::into_uninit(boxed)
    }
}

pub struct BoxReset;

impl<T> BoxCacheMethod<T> for BoxReset
where
    T: Reset,
{
    type Cached = T;

    fn into_cached(mut boxed: Box<T>) -> Box<Self::Cached> {
        boxed.reset();
        boxed
    }
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct BoxCache<T, M>
where
    M: BoxCacheMethod<T>,
{
    boxes: Stack<Box<M::Cached>>,
}

pub struct CachedBox<T, M>
where
    M: BoxCacheMethod<T>,
{
    cache: Rc<BoxCache<T, M>>,
    boxed: ManuallyDrop<Box<T>>,
}

impl<T, M> Deref for CachedBox<T, M>
where
    M: BoxCacheMethod<T>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.boxed
    }
}

impl<T, M> DerefMut for CachedBox<T, M>
where
    M: BoxCacheMethod<T>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.boxed
    }
}

impl<T> BoxCache<T, BoxUninit> {
    pub fn get(self: &Rc<Self>, unboxed: T) -> CachedBox<T, BoxUninit> {
        let c = self
            .boxes
            .pop()
            .unwrap_or_else(|| Box::new(MaybeUninit::uninit()));
        let c = Box::write(c, unboxed);
        CachedBox {
            cache: self.clone(),
            boxed: ManuallyDrop::new(c),
        }
    }
}

impl<T> BoxCache<T, BoxReset>
where
    T: Reset,
{
    pub fn get(self: &Rc<Self>) -> CachedBox<T, BoxReset> {
        let c = self.boxes.pop().unwrap_or_default();
        CachedBox {
            cache: self.clone(),
            boxed: ManuallyDrop::new(c),
        }
    }
}

impl<T> CachedBox<T, BoxReset>
where
    T: Reset,
{
    pub fn take(self: &mut CachedBox<T, BoxReset>) -> CachedBox<T, BoxReset> {
        mem::replace(self, self.cache.get())
    }
}

impl<T, M> Drop for CachedBox<T, M>
where
    M: BoxCacheMethod<T>,
{
    fn drop(&mut self) {
        let boxed = unsafe { ManuallyDrop::take(&mut self.boxed) };
        let boxed = M::into_cached(boxed);
        self.cache.boxes.push(boxed);
    }
}

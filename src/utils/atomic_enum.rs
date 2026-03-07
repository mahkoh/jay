use {
    linearize::Linearize,
    std::{
        marker::PhantomData,
        sync::atomic::{AtomicUsize, Ordering},
    },
};

pub struct AtomicEnum<T> {
    v: AtomicUsize,
    _phantom: PhantomData<T>,
}

impl<T> Default for AtomicEnum<T>
where
    T: Default + Linearize + Copy,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> AtomicEnum<T>
where
    T: Linearize + Copy,
{
    pub fn new(t: T) -> Self {
        Self {
            v: AtomicUsize::new(t.linearize()),
            _phantom: Default::default(),
        }
    }

    #[expect(dead_code)]
    pub fn load(&self, ordering: Ordering) -> T {
        unsafe { T::from_linear_unchecked(self.v.load(ordering)) }
    }

    #[expect(dead_code)]
    pub fn store(&self, t: T, ordering: Ordering) {
        self.v.store(t.linearize(), ordering);
    }
}

use {
    crate::utils::ptr_ext::{MutPtrExt, PtrExt},
    std::{cell::UnsafeCell, mem, ops::Deref},
};

pub struct RefCounted<T> {
    map: UnsafeCell<Vec<(T, usize)>>,
}

impl<T> Default for RefCounted<T> {
    fn default() -> Self {
        Self {
            map: UnsafeCell::new(vec![]),
        }
    }
}

impl<T: Eq> RefCounted<T> {
    pub fn add(&self, t: T) -> bool {
        unsafe {
            let map = self.map.get().deref_mut();
            for (k, v) in &mut *map {
                if k == &t {
                    *v += 1;
                    return false;
                }
            }
            map.push((t, 1));
            true
        }
    }

    pub fn remove(&self, t: &T) -> bool {
        unsafe {
            let map = self.map.get().deref_mut();
            let idx = 'idx: {
                for (idx, (k, v)) in map.iter_mut().enumerate() {
                    if k == t {
                        *v -= 1;
                        if *v > 0 {
                            return false;
                        } else {
                            break 'idx idx;
                        }
                    }
                }
                return false;
            };
            let _v = map.swap_remove(idx);
            true
        }
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Copy,
    {
        unsafe { self.map.get().deref().iter().map(|k| k.0).collect() }
    }

    pub fn lock(&self) -> Locked<'_, T> {
        unsafe {
            Locked {
                vec: mem::take(self.map.get().deref_mut()),
                rc: self,
            }
        }
    }
}

pub struct Locked<'a, T> {
    rc: &'a RefCounted<T>,
    vec: Vec<(T, usize)>,
}

impl<'a, T> Deref for Locked<'a, T> {
    type Target = [(T, usize)];

    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl<'a, T> Drop for Locked<'a, T> {
    fn drop(&mut self) {
        unsafe {
            *self.rc.map.get() = mem::take(&mut self.vec);
        }
    }
}

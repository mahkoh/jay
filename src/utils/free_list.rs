#[cfg(test)]
mod tests;

use {
    crate::utils::ptr_ext::MutPtrExt,
    std::{
        array,
        cell::UnsafeCell,
        fmt::{Debug, Formatter},
        marker::PhantomData,
    },
};

type Seg = usize;
const SEG_SIZE: usize = size_of::<Seg>() * 8;

pub struct FreeList<T, const N: usize> {
    levels: UnsafeCell<[Vec<Seg>; N]>,
    _phantom: PhantomData<T>,
}

impl<T, const N: usize> Default for FreeList<T, N> {
    fn default() -> Self {
        Self {
            levels: UnsafeCell::new(array::from_fn(|_| Vec::new())),
            _phantom: Default::default(),
        }
    }
}

impl<T, const N: usize> Debug for FreeList<T, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeList")
            .field("levels", self.get())
            .finish()
    }
}

impl<T, const N: usize> FreeList<T, N> {
    fn get(&self) -> &mut [Vec<Seg>; N] {
        unsafe { self.levels.get().deref_mut() }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn release(&self, n: T)
    where
        T: Into<u32>,
    {
        let mut ext = n.into() as usize;
        let mut int;
        let levels = self.get();
        assert!(ext / SEG_SIZE < levels[0].len());
        for level in self.get() {
            int = ext % SEG_SIZE;
            ext /= SEG_SIZE;
            unsafe {
                *level.get_unchecked_mut(ext) |= 1 << int;
            }
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn acquire(&self) -> T
    where
        u32: Into<T>,
    {
        let mut ext = 'last: {
            let level = &mut self.get()[N - 1];
            for (idx, &seg) in level.iter().enumerate() {
                if seg != 0 {
                    break 'last idx;
                }
            }
            level.len()
        };
        for level in self.get().iter_mut().rev() {
            if ext == level.len() {
                level.push(!0);
            }
            let seg = unsafe { level.get_unchecked(ext) };
            ext = SEG_SIZE * ext + seg.trailing_zeros() as usize;
        }
        let id = ext as u32;
        for level in self.get().iter_mut() {
            let int = ext % SEG_SIZE;
            ext /= SEG_SIZE;
            let seg = unsafe { level.get_unchecked_mut(ext) };
            *seg &= !(1 << int);
            if *seg != 0 {
                break;
            }
        }
        id.into()
    }
}

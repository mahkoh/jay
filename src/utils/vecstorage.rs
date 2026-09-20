use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;

pub struct VecStorage<T> {
    ptr: *mut T,
    cap: usize,
}

impl<T> Default for VecStorage<T> {
    fn default() -> Self {
        let mut v = ManuallyDrop::new(vec![]);
        Self {
            ptr: v.as_mut_ptr(),
            cap: v.capacity(),
        }
    }
}

impl<T> VecStorage<T> {
    pub fn take(&mut self) -> RealizedVec<'_, T, T> {
        self.take_as()
    }

    pub fn take_as<U>(&mut self) -> RealizedVec<'_, T, U> {
        assert_size_eq!(T, U);
        assert_align_eq!(T, U);
        unsafe {
            RealizedVec {
                vec: ManuallyDrop::new(self.to_vector()),
                storage: self,
            }
        }
    }

    unsafe fn to_vector<U>(&mut self) -> Vec<U> {
        unsafe { Vec::from_raw_parts(self.ptr as _, 0, self.cap) }
    }
}

impl<T> Drop for VecStorage<T> {
    fn drop(&mut self) {
        unsafe {
            drop(self.to_vector::<T>());
        }
    }
}

pub struct RealizedVec<'a, T, U> {
    vec: ManuallyDrop<Vec<U>>,
    storage: &'a mut VecStorage<T>,
}

impl<T, U> Drop for RealizedVec<'_, T, U> {
    fn drop(&mut self) {
        self.vec.clear();
        self.storage.ptr = self.vec.as_mut_ptr() as _;
        self.storage.cap = self.vec.capacity();
    }
}

impl<T, U> Deref for RealizedVec<'_, T, U> {
    type Target = Vec<U>;

    fn deref(&self) -> &Self::Target {
        self.vec.deref()
    }
}

impl<T, U> DerefMut for RealizedVec<'_, T, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.vec.deref_mut()
    }
}

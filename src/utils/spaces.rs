use crate::utils::numcell::NumCell;
use crate::utils::ptr_ext::PtrExt;
use isnt::std_1::primitive::IsntMutPtrExt;
use std::cell::Cell;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Deref;
use std::ptr;

#[path = "./spaces/tests.rs"]
#[cfg(test)]
mod tests;

pub struct Spaces {
    storage: *mut Storage,
    s: *const str,
}

struct Storage {
    string: Box<str>,
    rc: NumCell<usize>,
}

thread_local! {
    static STORAGE: Cell<*mut Storage> = const { Cell::new(ptr::null_mut()) };
}

impl Storage {
    unsafe fn dec(slf: *mut Self) {
        let rc = unsafe { slf.deref().rc.sub_fetch(1) };
        if rc == 0 {
            Self::free(slf)
        }
    }

    #[cold]
    fn free(slf: *mut Self) {
        unsafe {
            drop(Box::from_raw(slf));
        }
    }
}

pub fn spaces(n: usize) -> Spaces {
    let ptr = STORAGE.get();
    if ptr.is_not_null() {
        let storage = unsafe { ptr.deref() };
        if let Some(s) = storage.string.get(..n) {
            storage.rc.fetch_add(1);
            return Spaces { storage: ptr, s };
        }
    }
    slow(n)
}

#[cold]
fn slow(n: usize) -> Spaces {
    let storage = Box::into_raw(Box::new(Storage {
        string: " ".repeat(n).into_boxed_str(),
        rc: NumCell::new(2),
    }));
    let old = STORAGE.replace(storage);
    if old.is_not_null() {
        unsafe {
            Storage::dec(old);
        }
    }
    Spaces {
        storage,
        s: unsafe { &raw const *storage.deref().string },
    }
}

impl Drop for Spaces {
    fn drop(&mut self) {
        unsafe {
            Storage::dec(self.storage);
        }
    }
}

impl Deref for Spaces {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        unsafe { self.s.deref() }
    }
}

impl Display for Spaces {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.deref())
    }
}

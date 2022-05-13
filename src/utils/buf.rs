use {
    crate::utils::{numcell::NumCell, ptr_ext::PtrExt},
    std::{
        alloc::Layout,
        collections::Bound,
        ops::{Deref, DerefMut, Range, RangeBounds},
        ptr::NonNull,
        slice,
    },
};

const METADATA_SIZE: u32 = 8;
const METADATA_ALIGN: usize = 4;
const RC_OFF: u32 = 4;
const RC_OFF_INV: u32 = METADATA_SIZE - RC_OFF;

pub struct Buf {
    storage: NonNull<u8>,
    range: Range<u32>,
}

impl Buf {
    pub fn from_slice(vec: &[u8]) -> Buf {
        let len = vec.len();
        assert!(len <= (u32::MAX - METADATA_SIZE) as usize);
        let len = len as u32;
        let size = len + METADATA_SIZE;
        let layout = Layout::from_size_align(size as _, METADATA_ALIGN).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        unsafe {
            *ptr.cast::<u32>() = size;
            *ptr.add(RC_OFF as _).cast::<u32>() = 1;
            let mut buf = Buf {
                storage: NonNull::new_unchecked(ptr.add(METADATA_SIZE as _)),
                range: Range { start: 0, end: len },
            };
            buf[..].copy_from_slice(vec);
            buf
        }
    }

    pub fn new(len: usize) -> Buf {
        assert!(len <= (u32::MAX - METADATA_SIZE) as usize);
        let len = len as u32;
        let size = len + METADATA_SIZE;
        let layout = Layout::from_size_align(size as _, METADATA_ALIGN).unwrap();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        unsafe {
            *ptr.cast::<u32>() = size;
            *ptr.add(RC_OFF as _).cast::<u32>() = 1;
            Buf {
                storage: NonNull::new_unchecked(ptr.add(METADATA_SIZE as _)),
                range: Range { start: 0, end: len },
            }
        }
    }

    pub fn clone(&mut self) -> Buf {
        self.rc().fetch_add(1);
        Buf {
            storage: self.storage,
            range: self.range.clone(),
        }
    }

    pub fn slice(&mut self, range: impl RangeBounds<usize>) -> Buf {
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.wrapping_add(1),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n.wrapping_add(1),
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.len(),
        };
        self.slice_(start as _, end as _)
    }

    fn slice_(&mut self, start: u32, end: u32) -> Buf {
        assert!(start <= end);
        assert!(end <= self.len32());
        self.rc().fetch_add(1);
        Buf {
            storage: self.storage,
            range: Range {
                start: self.range.start + start,
                end: self.range.start + end,
            },
        }
    }

    fn rc(&self) -> &NumCell<u32> {
        unsafe {
            self.storage
                .as_ptr()
                .sub(RC_OFF_INV as _)
                .cast::<NumCell<u32>>()
                .deref()
        }
    }

    fn assert_unique(&self) {
        assert_eq!(self.rc().get(), 1);
    }

    pub fn len32(&self) -> u32 {
        self.range.end - self.range.start
    }

    pub fn len(&self) -> usize {
        self.len32() as _
    }

    pub fn as_ptr(&self) -> *mut u8 {
        unsafe { self.storage.as_ptr().add(self.range.start as _) }
    }
}

impl Deref for Buf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.assert_unique();
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len()) }
    }
}

impl DerefMut for Buf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.assert_unique();
        unsafe { slice::from_raw_parts_mut(self.as_ptr(), self.len()) }
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        unsafe {
            let prev = self.rc().fetch_sub(1);
            if prev != 1 {
                return;
            }
            let ptr = self.storage.as_ptr().sub(METADATA_SIZE as _).cast::<u32>();
            let size = *ptr as _;
            let layout = Layout::from_size_align_unchecked(size, METADATA_ALIGN);
            std::alloc::dealloc(ptr as _, layout);
        }
    }
}

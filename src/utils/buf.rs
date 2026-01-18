use {
    crate::utils::{numcell::NumCell, ptr_ext::PtrExt},
    std::{
        alloc::Layout,
        cmp,
        collections::Bound,
        fmt::Arguments,
        io::{self, Write},
        marker::PhantomData,
        mem,
        ops::{Deref, DerefMut, Range, RangeBounds},
        ptr::NonNull,
        slice,
    },
    uapi::Pod,
};

const METADATA_SIZE: u32 = 8;
const METADATA_ALIGN: usize = 4;
const SIZE_OFF: u32 = 0;
const RC_OFF: u32 = 4;
const RC_OFF_INV: u32 = METADATA_SIZE - RC_OFF;
const SIZE_OFF_INV: u32 = METADATA_SIZE - SIZE_OFF;

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

    fn size32(&self) -> u32 {
        unsafe {
            *self
                .storage
                .as_ptr()
                .sub(SIZE_OFF_INV as _)
                .cast::<u32>()
                .deref()
        }
    }

    pub fn cap32(&self) -> u32 {
        self.size32() - METADATA_SIZE
    }

    pub fn as_ptr(&self) -> *mut u8 {
        unsafe { self.storage.as_ptr().add(self.range.start as _) }
    }

    #[expect(dead_code)]
    pub fn write_fmt(&mut self, args: Arguments) -> Result<Self, io::Error> {
        let cap = self.len();
        let mut buf = self.deref_mut();
        buf.write_fmt(args)?;
        let len = cap - buf.len();
        Ok(self.slice(..len))
    }

    pub fn into_full(self) -> Self {
        let new = Self {
            storage: self.storage,
            range: 0..self.cap32(),
        };
        mem::forget(self);
        new
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.as_ptr(), self.len()) }
    }
}

impl Default for Buf {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Deref for Buf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.assert_unique();
        self.as_slice()
    }
}

impl DerefMut for Buf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.assert_unique();
        self.as_slice_mut()
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

pub struct DynamicBuf {
    buf: Buf,
    len: usize,
}

impl DynamicBuf {
    pub fn new() -> Self {
        Self {
            buf: Buf::new(0),
            len: 0,
        }
    }

    pub fn from_buf(buf: Buf) -> Self {
        buf.assert_unique();
        Self {
            buf: buf.into_full(),
            len: 0,
        }
    }

    pub fn unwrap(mut self) -> Buf {
        self.buf.slice(..self.len)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn reserve(&mut self, n: usize) {
        if self.buf.len() - self.len < n {
            let cap = self.len.checked_add(n).unwrap();
            let cap = cmp::max(self.buf.len() * 2, cap);
            let mut new = Buf::new(cap);
            new[..self.len].copy_from_slice(&self.buf[..self.len]);
            self.buf = new;
        }
    }

    pub fn extend_from_slice(&mut self, buf: &[u8]) {
        self.reserve(buf.len());
        self.buf.as_slice_mut()[self.len..self.len + buf.len()].copy_from_slice(buf);
        self.len += buf.len();
    }

    pub fn push(&mut self, b: u8) {
        self.extend_from_slice(&[b]);
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn borrow(&mut self) -> BorrowedBuf<'_> {
        BorrowedBuf {
            buf: self.buf.slice(..self.len),
            _phantom: Default::default(),
        }
    }
}

pub struct BorrowedBuf<'a> {
    pub buf: Buf,
    _phantom: PhantomData<&'a mut DynamicBuf>,
}

impl<'a> Drop for BorrowedBuf<'a> {
    fn drop(&mut self) {
        assert_eq!(self.buf.rc().get(), 2);
    }
}

impl Write for DynamicBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Deref for DynamicBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buf.as_slice()
    }
}

impl DerefMut for DynamicBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buf.as_slice_mut()
    }
}

pub struct TypedBuf<T: Pod> {
    buf: Buf,
    _phantom: PhantomData<T>,
}

impl<T: Pod> TypedBuf<T> {
    pub fn new() -> Self {
        Self {
            buf: Buf::new(size_of::<T>()),
            _phantom: Default::default(),
        }
    }

    pub fn buf(&mut self) -> Buf {
        self.buf.clone()
    }

    pub fn t(&self) -> T {
        uapi::pod_read(&self.buf[..]).unwrap()
    }
}

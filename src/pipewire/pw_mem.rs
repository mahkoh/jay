use {
    crate::utils::{
        copyhashmap::CopyHashMap,
        mmap::{mmap, Mmapped},
        oserror::OsError,
        page_size::page_size,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    std::{marker::PhantomData, mem, ops::Range, rc::Rc},
    thiserror::Error,
    uapi::{c, OwnedFd, Pod},
};

#[derive(Default)]
pub struct PwMemPool {
    pub mems: CopyHashMap<u32, Rc<PwMem>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PwMemType {
    MemFd,
    DmaBuf,
}

pub struct PwMem {
    pub ty: PwMemType,
    pub read: bool,
    pub write: bool,
    pub fd: Rc<OwnedFd>,
}

pub struct PwMemMap {
    pub mem: Rc<PwMem>,
    pub range: Range<usize>,
    pub map: Mmapped,
}

pub struct PwMemTyped<T> {
    mem: Rc<PwMemMap>,
    offset: usize,
    _phantom: PhantomData<T>,
}

impl PwMemPool {
    pub fn map(&self, memid: u32, offset: u32, size: u32) -> Result<Rc<PwMemMap>, PwMemError> {
        match self.mems.get(&memid) {
            Some(m) => m.map(offset, size),
            _ => Err(PwMemError::MemidDoesNotExist(memid)),
        }
    }
}

impl PwMem {
    pub fn map(self: &Rc<Self>, offset: u32, size: u32) -> Result<Rc<PwMemMap>, PwMemError> {
        let mask = page_size() - 1;
        let offset = offset as usize;
        let size = size as usize;
        let start = offset & !mask;
        let dist = offset - start;
        let len = (size + dist + mask) & !mask;
        let range = dist..dist + size;
        let mut prot = 0;
        if self.read {
            prot |= c::PROT_READ;
        }
        if self.write {
            prot |= c::PROT_WRITE;
        }
        let map = match mmap(len as _, prot, c::MAP_SHARED, self.fd.raw(), start as _) {
            Ok(m) => m,
            Err(e) => return Err(PwMemError::MmapFailed(e)),
        };
        Ok(Rc::new(PwMemMap {
            mem: self.clone(),
            range,
            map,
        }))
    }
}

impl PwMemMap {
    pub unsafe fn read<T: Pod>(&self) -> &T {
        self.check::<T>(0);
        (self.map.ptr.cast::<u8>().add(self.range.start) as *const T).deref()
    }

    pub unsafe fn write<T: Pod>(&self) -> &mut T {
        self.check::<T>(0);
        (self.map.ptr.cast::<u8>().add(self.range.start) as *mut T).deref_mut()
    }

    pub unsafe fn bytes_mut(&self) -> &mut [u8] {
        std::slice::from_raw_parts_mut(
            self.map.ptr.cast::<u8>().add(self.range.start) as _,
            self.range.len(),
        )
    }

    fn check<T>(&self, offset: usize) {
        assert!(offset <= self.range.len());
        assert!(mem::size_of::<T>() <= self.range.len() - offset);
        assert_eq!((mem::align_of::<T>() - 1) & (self.range.start + offset), 0);
    }

    pub fn typed<T: Pod>(self: &Rc<Self>) -> Rc<PwMemTyped<T>> {
        self.typed_at(0)
    }

    pub fn typed_at<T: Pod>(self: &Rc<Self>, offset: usize) -> Rc<PwMemTyped<T>> {
        self.check::<T>(offset);
        Rc::new(PwMemTyped {
            mem: self.clone(),
            offset: self.range.start + offset,
            _phantom: Default::default(),
        })
    }
}

impl<T: Pod> PwMemTyped<T> {
    pub unsafe fn read(&self) -> &T {
        (self.mem.map.ptr.cast::<u8>().add(self.offset) as *const T).deref()
    }

    pub unsafe fn write(&self) -> &mut T {
        (self.mem.map.ptr.cast::<u8>().add(self.offset) as *mut T).deref_mut()
    }
}

#[derive(Debug, Error)]
pub enum PwMemError {
    #[error("mmap failed")]
    MmapFailed(#[source] OsError),
    #[error("memid {0} does not exist")]
    MemidDoesNotExist(u32),
}

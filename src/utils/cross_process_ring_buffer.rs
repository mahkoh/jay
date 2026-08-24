use crate::io_uring::FutexObj;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
use crate::utils::array;
use crate::utils::bitflags::BitflagsExt;
use crate::utils::mmap::Mmapped;
use crate::utils::mmap::mmap;
use crate::utils::numcell::NumCell;
use crate::utils::ptr_ext::PtrExt;
use derivative::Derivative;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt2;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Acquire;
use std::sync::atomic::Ordering::Release;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;
use uapi::fcntl_add_seals;
use uapi::ftruncate;
use uapi::memfd_create;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum CprbError {
    #[error("Could not create a memfd")]
    CreateMemfd(#[source] OsError),
    #[error("Could not truncate the memfd")]
    TruncateMemfd(#[source] OsError),
    #[error("Could not seal the memfd")]
    SealMemfd(#[source] OsError),
    #[error("Could not map the memfd")]
    MapMemfd(#[source] OsError),
    #[error("Could not get memfd seals")]
    GetSeals(#[source] OsError),
    #[error("Memfd has no shrink seal")]
    NoShrinkSeal,
    #[error("Could not stat memfd")]
    Stat(#[source] OsError),
    #[error("Memfd is too small")]
    Size,
}

pub trait Cprb: 'static {
    type Slot;
    type Data;
}

pub struct CprbWrite<T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    memfd: Rc<OwnedFd>,
    next_serial: NumCell<u64>,
    shared: Rc<SharedMapping<T, NUM_SLOTS>>,
    next_slot: NumCell<u64>,
    data_offsets: [Cell<u64>; NUM_SLOTS],
    data_offsets_idx: NumCell<u64>,
}

pub struct CprbRead<T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    shared: Rc<SharedMapping<T, NUM_SLOTS>>,
    next_slot: NumCell<u64>,
    last_serial: u64,
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct CprbReadAvailable<T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    shared: Rc<SharedMapping<T, NUM_SLOTS>>,
}

#[repr(align(64))]
struct AlignedAtomic {
    v: AtomicU32,
}

struct SharedMapping<T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    ring: Rc<IoUring>,
    _mapping: Mmapped,
    shared: *mut Shared<T, NUM_SLOTS>,
}

struct Shared<T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    available: AlignedAtomic,
    slots: [Slot<T>; NUM_SLOTS],
    data: T::Data,
}

struct Slot<T>
where
    T: Cprb,
{
    serial: u64,
    data: T::Slot,
}

pub struct CprbMsgRead<'a, T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    src: &'a CprbRead<T, NUM_SLOTS>,
    pub slot: *const T::Slot,
    pub missed: u64,
}

pub struct CprbMsgWrite<'a, T, const NUM_SLOTS: usize>
where
    T: Cprb,
{
    _src: &'a CprbWrite<T, NUM_SLOTS>,
    pub slot: *mut T::Slot,
    pub read: u64,
    pub write: u64,
}

impl<T, const NUM_SLOTS: usize> SharedMapping<T, NUM_SLOTS>
where
    T: Cprb,
{
    fn available(&self) -> &AtomicU32 {
        unsafe { (&raw const (*self.shared).available.v).deref() }
    }

    fn slots(&self) -> *mut Slot<T> {
        unsafe { (&raw mut (*self.shared).slots).cast() }
    }

    fn data(&self) -> *mut T::Data {
        unsafe { &raw mut (*self.shared).data }
    }
}

impl<T, const NUM_SLOTS: usize> FutexObj for SharedMapping<T, NUM_SLOTS>
where
    T: Cprb,
{
    fn get(&self) -> &AtomicU32 {
        self.available()
    }
}

impl<T, const NUM_SLOTS: usize> CprbWrite<T, NUM_SLOTS>
where
    T: Cprb,
{
    pub fn new(ring: &Rc<IoUring>) -> Result<Self, CprbError> {
        let size = size_of::<Shared<T, NUM_SLOTS>>();
        let memfd = memfd_create("jay-cprb", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING)
            .map_os_err(CprbError::CreateMemfd)?;
        ftruncate(memfd.raw(), size as _).map_os_err(CprbError::TruncateMemfd)?;
        fcntl_add_seals(memfd.raw(), c::F_SEAL_SHRINK).map_os_err(CprbError::SealMemfd)?;
        let mapping = mmap(
            size,
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED,
            memfd.raw(),
            0,
        )
        .map_err(CprbError::MapMemfd)?;
        Ok(Self {
            memfd: Rc::new(memfd),
            next_serial: NumCell::new(1),
            shared: Rc::new(SharedMapping {
                ring: ring.clone(),
                shared: mapping.ptr.cast_mut().cast(),
                _mapping: mapping,
            }),
            next_slot: Default::default(),
            data_offsets: array::from_fn(|_| Default::default()),
            data_offsets_idx: NumCell::new(NUM_SLOTS as u64),
        })
    }

    pub fn memfd(&self) -> &Rc<OwnedFd> {
        &self.memfd
    }

    pub fn data(&self) -> *mut T::Data {
        self.shared.data()
    }

    fn data_offset(&self, idx: u64) -> &Cell<u64> {
        let idx = (idx % (NUM_SLOTS as u64)) as usize;
        &self.data_offsets[idx]
    }

    pub fn acquire(&self) -> Option<CprbMsgWrite<'_, T, NUM_SLOTS>> {
        let serial = self.next_serial.fetch_add(1);
        let available = self.shared.available().load(Acquire) as u64;
        if available >= NUM_SLOTS as u64 {
            return None;
        }
        let hi = self.data_offsets_idx.get();
        let write = self.data_offset(hi).get();
        let read = self.data_offset(hi - available).get();
        let slot_idx = self.next_slot.get() % (NUM_SLOTS as u64);
        let slot = unsafe { self.shared.slots().add(slot_idx as usize) };
        unsafe {
            (&raw mut (*slot).serial).write(serial);
        }
        let slot = unsafe { &raw mut (*slot).data };
        Some(CprbMsgWrite {
            _src: self,
            slot,
            read,
            write,
        })
    }

    #[inline]
    pub fn commit(&self, data_offset: u64) {
        let hi = self.data_offsets_idx.add_fetch(1);
        self.data_offset(hi).set(data_offset);
        self.next_slot.fetch_add(1);
        if self.shared.available().fetch_add(1, Release) == 0 {
            #[cold]
            fn cold() {}
            cold();
            atomic::fence(Acquire);
            let _ = self.shared.ring.futex_wake(&self.shared, i32::MAX, false);
        }
    }
}

impl<T, const NUM_SLOTS: usize> Drop for CprbMsgRead<'_, T, NUM_SLOTS>
where
    T: Cprb,
{
    fn drop(&mut self) {
        self.src.shared.available().fetch_sub(1, Release);
    }
}

impl<T, const NUM_SLOTS: usize> CprbRead<T, NUM_SLOTS>
where
    T: Cprb,
{
    pub fn new(
        ring: &Rc<IoUring>,
        memfd: &Rc<OwnedFd>,
    ) -> Result<CprbRead<T, NUM_SLOTS>, CprbError> {
        let seals = uapi::fcntl_get_seals(memfd.raw()).map_os_err(CprbError::GetSeals)?;
        if seals.not_contains(c::F_SEAL_SHRINK) {
            return Err(CprbError::NoShrinkSeal);
        }
        let stat = uapi::fstat(memfd.raw()).map_os_err(CprbError::Stat)?;
        if (stat.st_size as u64) < size_of::<Shared<T, NUM_SLOTS>>() as u64 {
            return Err(CprbError::Size);
        }
        let mapping = mmap(
            size_of::<Shared<T, NUM_SLOTS>>(),
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED,
            memfd.raw(),
            0,
        )
        .map_err(CprbError::MapMemfd)?;
        Ok(CprbRead {
            shared: Rc::new(SharedMapping {
                ring: ring.clone(),
                shared: mapping.ptr.cast_mut().cast(),
                _mapping: mapping,
            }),
            next_slot: Default::default(),
            last_serial: Default::default(),
        })
    }

    pub fn data(&self) -> *mut T::Data {
        self.shared.data()
    }

    pub fn available(&self) -> CprbReadAvailable<T, NUM_SLOTS> {
        CprbReadAvailable {
            shared: self.shared.clone(),
        }
    }

    pub fn acquire(&mut self) -> Option<CprbMsgRead<'_, T, NUM_SLOTS>> {
        let available = self.shared.available().load(Acquire) as usize;
        if available == 0 {
            return None;
        }
        let slot = self.next_slot.fetch_add(1) % (NUM_SLOTS as u64);
        let slot = unsafe { self.shared.slots().add(slot as usize) };
        let serial = unsafe { (&raw const (*slot).serial).read() };
        let slot = unsafe { &raw const (*slot).data };
        let missed = serial - self.last_serial - 1;
        self.last_serial = serial;
        Some(CprbMsgRead {
            src: self,
            slot,
            missed,
        })
    }
}

impl<T, const NUM_SLOTS: usize> CprbReadAvailable<T, NUM_SLOTS>
where
    T: Cprb,
{
    pub async fn available(&self) -> Result<(), IoUringError> {
        loop {
            let available = self.shared.available().load(Acquire) as usize;
            if available == 0 {
                self.shared.ring.futex_wait(&self.shared, 0, false).await?;
            } else {
                return Ok(());
            }
        }
    }
}

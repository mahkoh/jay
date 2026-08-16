use crate::io_uring::IoUring;
use crate::io_uring::IoUringData;
use crate::io_uring::IoUringError;
use crate::io_uring::sys::IORING_CQE_BUFFER_SHIFT;
use crate::io_uring::sys::IORING_CQE_F_BUFFER;
use crate::io_uring::sys::IORING_OFF_PBUF_RING;
use crate::io_uring::sys::IORING_OFF_PBUF_SHIFT;
use crate::io_uring::sys::IORING_REGISTER_PBUF_RING;
use crate::io_uring::sys::IORING_UNREGISTER_PBUF_RING;
use crate::io_uring::sys::IOU_PBUF_RING_MMAP;
use crate::io_uring::sys::io_uring_buf;
use crate::io_uring::sys::io_uring_buf_reg;
use crate::io_uring::sys::io_uring_cqe;
use crate::io_uring::sys::io_uring_register;
use crate::utils::bitflags::BitflagsExt;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use jay_algorithms::mmap::Mmapped;
use jay_algorithms::mmap::mmap;
use jay_algorithms::oserror::OsError;
use run_on_drop::on_drop;
use std::alloc;
use std::alloc::Layout;
use std::alloc::LayoutError;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr;
use std::rc::Rc;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::Ordering::Release;
use thiserror::Error;
use uapi::c;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum BufferRingError {
    #[error("Buffer ring does not belong to this io-uring")]
    DifferentRing,
    #[error("No available buffer ring id")]
    Ids,
    #[error("Buffer ring entry size must be greater than 0")]
    ZeroSize,
    #[error("Buffer ring entry size overflowed")]
    SizeOverflow,
    #[error("Too many entries requested")]
    TooManyEntries,
    #[error("Could not calculate buffer ring layout")]
    Layout(#[source] LayoutError),
    #[error("Buffer ring allocation failed")]
    Allocation,
    #[error("Could not register buffer ring")]
    Register(#[source] OsError),
    #[error("Could not map buffer ring")]
    Map(#[source] OsError),
}

pub struct BufferRing {
    _mmapped: Mmapped,
    ctrl: *mut io_uring_buf,
    mask: usize,
    pub(super) ring: Rc<IoUringData>,
    pub(super) bgid: u16,
    buf: *mut u8,
    stride: usize,
    len: usize,
    buf_layout: Layout,
}

pub struct BufferRingBuffer {
    ring: Option<Rc<BufferRing>>,
    buf: *mut [u8],
    idx: usize,
}

impl IoUring {
    pub fn create_buffer_ring(
        self: &Rc<Self>,
        num: usize,
        each_size: usize,
        alignment: usize,
    ) -> Result<Rc<BufferRing>, IoUringError> {
        self.ring.check_destroyed()?;
        let ring = self.create_buffer_ring_(num, each_size, alignment)?;
        Ok(ring)
    }

    fn create_buffer_ring_(
        self: &Rc<Self>,
        num: usize,
        each_size: usize,
        alignment: usize,
    ) -> Result<Rc<BufferRing>, BufferRingError> {
        if each_size == 0 {
            return Err(BufferRingError::ZeroSize);
        }
        let num = num
            .checked_next_power_of_two()
            .ok_or(BufferRingError::TooManyEntries)?;
        if each_size > u32::MAX as usize {
            return Err(BufferRingError::SizeOverflow);
        }
        let bgid = self.ring.buffer_ring_ids.borrow_mut().acquire();
        let release_id = on_drop(|| self.ring.buffer_ring_ids.borrow_mut().release(bgid));
        if bgid > u16::MAX as u32 {
            return Err(BufferRingError::Ids);
        }
        let bgid = bgid as u16;
        let (layout, stride) = Layout::from_size_align(each_size, alignment)
            .and_then(|layout| layout.repeat(num))
            .map_err(BufferRingError::Layout)?;
        let buf = unsafe { alloc::alloc(layout) };
        if buf.is_null() {
            return Err(BufferRingError::Allocation);
        }
        let dealloc = on_drop(|| unsafe { alloc::dealloc(buf, layout) });
        let mut req = io_uring_buf_reg {
            ring_addr: 0,
            ring_entries: num
                .try_into()
                .map_err(|_| BufferRingError::TooManyEntries)?,
            bgid,
            flags: IOU_PBUF_RING_MMAP,
            min_left: 0,
            resv: [0; 5],
        };
        unsafe {
            io_uring_register(
                self.ring.fd.raw(),
                IORING_REGISTER_PBUF_RING,
                ptr::from_mut(&mut req).cast(),
                1,
            )
            .map_err(BufferRingError::Register)?;
        }
        let unregister = on_drop(|| unregister_ring(&self.ring, bgid));
        let mapping_size = num * size_of::<io_uring_buf>();
        let mapping_off =
            IORING_OFF_PBUF_RING as c::off_t | ((bgid as c::off_t) << IORING_OFF_PBUF_SHIFT);
        let mmapped = mmap(
            mapping_size,
            c::PROT_READ | c::PROT_WRITE,
            c::MAP_SHARED | c::MAP_POPULATE,
            self.ring.fd.raw(),
            mapping_off,
        )
        .map_err(BufferRingError::Map)?;
        unregister.forget();
        dealloc.forget();
        release_id.forget();
        let ring = Rc::new(BufferRing {
            ctrl: mmapped.ptr.cast_mut().cast(),
            _mmapped: mmapped,
            mask: num - 1,
            ring: self.ring.clone(),
            bgid,
            buf,
            stride,
            len: each_size,
            buf_layout: layout,
        });
        for idx in 0..num {
            unsafe {
                ring.add_buffer(idx);
            }
        }
        Ok(ring)
    }
}

impl Drop for BufferRing {
    fn drop(&mut self) {
        unregister_ring(&self.ring, self.bgid);
        self.ring
            .buffer_ring_ids
            .borrow_mut()
            .release(self.bgid as u32);
        unsafe {
            alloc::dealloc(self.buf, self.buf_layout);
        }
    }
}

fn unregister_ring(ring: &IoUringData, bgid: u16) {
    let mut req = io_uring_buf_reg {
        ring_addr: 0,
        ring_entries: 0,
        bgid,
        flags: 0,
        min_left: 0,
        resv: [0; 5],
    };
    let res = unsafe {
        io_uring_register(
            ring.fd.raw(),
            IORING_UNREGISTER_PBUF_RING,
            ptr::from_mut(&mut req).cast(),
            1,
        )
    };
    if let Err(e) = res {
        log::error!("Could not unregister buffer ring: {}", ErrorFmt(e));
    }
}

impl BufferRing {
    pub(super) fn check_ring(&self, ring: &IoUring) -> Result<(), BufferRingError> {
        if !Rc::ptr_eq(&self.ring, &ring.ring) {
            return Err(BufferRingError::DifferentRing);
        }
        Ok(())
    }

    unsafe fn add_buffer(&self, idx: usize) {
        let tail = unsafe { AtomicU16::from_ptr(&raw mut (*self.ctrl).resv) };
        let tail_val = tail.load(Relaxed);
        let offset = tail_val as usize & self.mask;
        unsafe {
            let ctrl = self.ctrl.add(offset);
            let addr = &raw mut (*ctrl).addr;
            addr.write(self.buf.add(self.stride * idx) as u64);
            let len = &raw mut (*ctrl).len;
            len.write(self.len as u32);
            let bid = &raw mut (*ctrl).bid;
            bid.write(idx as u16);
        }
        tail.store(tail_val.wrapping_add(1), Release);
    }

    pub(super) unsafe fn get_buffer(self: &Rc<Self>, cqe: &io_uring_cqe) -> BufferRingBuffer {
        if cqe.flags.not_contains(IORING_CQE_F_BUFFER) {
            return BufferRingBuffer {
                ring: None,
                buf: &mut [],
                idx: 0,
            };
        }
        let len = cqe.res.max(0) as usize;
        let idx = (cqe.flags >> IORING_CQE_BUFFER_SHIFT) as usize;
        let buf_lo = unsafe { self.buf.add(self.stride * idx) };
        let buf = ptr::slice_from_raw_parts_mut(buf_lo, len);
        BufferRingBuffer {
            ring: Some(self.clone()),
            buf,
            idx,
        }
    }
}

impl Drop for BufferRingBuffer {
    fn drop(&mut self) {
        if let Some(ring) = &self.ring {
            unsafe {
                ring.add_buffer(self.idx);
            }
        }
    }
}

impl BufferRingBuffer {
    #[cfg_attr(not(test), expect(unused))]
    pub fn as_ptr(&self) -> *const u8 {
        self.buf.cast()
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.buf.cast()
    }
}

impl Deref for BufferRingBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { self.buf.deref() }
    }
}

impl DerefMut for BufferRingBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.buf.deref_mut() }
    }
}

use crate::backend::DrmDeviceId;
use crate::format::Format;
use crate::utils::unique_process_id::unique_process_id;
use crate::video::Modifier;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use crate::video::drm::Drm;
use dmabuf_allocator::Allocation;
use std::cell::Cell;
use std::ffi::CString;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::mem::DropGuard;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::format_ustr;

#[derive(Debug, Error)]
pub enum AllocatorError {
    #[error("Could not create the allocator")]
    CreateAllocator(#[source] dmabuf_allocator::Error),
    #[error("Could not allocate the buffer")]
    Allocate(#[source] dmabuf_allocator::Error),
}

bitflags! {
    BufferUsage: u32;
        BO_USE_SCANOUT,
        BO_USE_LINEAR,
}

pub struct Allocator {
    alloc: dmabuf_allocator::Allocator,
    buffer_name: CString,
}

impl Debug for Allocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Allocator").finish_non_exhaustive()
    }
}

impl Allocator {
    pub fn from_drm(drm: &Drm) -> Result<Self, AllocatorError> {
        Self::new(Some(drm.id()), drm.fd())
    }

    pub fn new(_dev: Option<DrmDeviceId>, fd: &OwnedFd) -> Result<Self, AllocatorError> {
        Ok(Self {
            alloc: dmabuf_allocator::Allocator::new(fd.raw())
                .map_err(AllocatorError::CreateAllocator)?,
            buffer_name: format_ustr!("jay-{}", unique_process_id())
                .into_c_string()
                .unwrap(),
        })
    }

    pub fn allocate(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: impl IntoIterator<Item = Modifier>,
    ) -> Result<Rc<DmaBuf>, AllocatorError> {
        self.allocate_with_usage(
            dma_buf_ids,
            width,
            height,
            format,
            modifiers,
            BufferUsage::none(),
        )
    }

    pub fn allocate_with_usage(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: impl IntoIterator<Item = Modifier>,
        usage: BufferUsage,
    ) -> Result<Rc<DmaBuf>, AllocatorError> {
        thread_local! {
            static MOD_CACHE: Cell<Vec<Modifier>> = const { Cell::new(Vec::new()) };
        }
        let mut mods = DropGuard::new(MOD_CACHE.take(), |m| MOD_CACHE.set(m));
        mods.clear();
        mods.extend(modifiers);
        self.allocate_(dma_buf_ids, width, height, format, &mods, usage)
    }

    fn allocate_(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: &[Modifier],
        usage: BufferUsage,
    ) -> Result<Rc<DmaBuf>, AllocatorError> {
        let Allocation { layout, dmabuf, .. } = self
            .alloc
            .builder()
            .format(format.da_format)
            .modifiers(&modifiers)
            .size(width as u32, height as u32)
            .name(&self.buffer_name)
            .implicit_sync(true)
            .scanout(usage.contains(BO_USE_SCANOUT))
            .implicit_linear(usage.contains(BO_USE_LINEAR))
            .allocate()
            .map_err(AllocatorError::Allocate)?;
        let fd = Rc::new(OwnedFd::from(dmabuf.into_std()));
        let mut planes = PlaneVec::new();
        for p in layout.planes() {
            planes.push(DmaBufPlane {
                offset: p.offset() as u32,
                stride: p.stride() as u32,
                fd: fd.clone(),
            });
        }
        let dmabuf = DmaBuf::new(
            dma_buf_ids,
            width,
            height,
            format,
            layout.modifier(),
            planes,
        );
        Ok(dmabuf)
    }
}

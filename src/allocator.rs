use crate::format::Format;
use crate::gfx_api::SyncFile;
use crate::video::Modifier;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::drm::Drm;
use std::error::Error;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct AllocatorError(#[from] pub Box<dyn Error + Send + Sync>);

bitflags! {
    BufferUsage: u32;
        BO_USE_SCANOUT,
        BO_USE_CURSOR,
        BO_USE_RENDERING,
        BO_USE_WRITE,
        BO_USE_LINEAR,
        BO_USE_PROTECTED,
}

pub trait Allocator {
    fn drm(&self) -> Option<&Drm>;
    fn create_bo(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: &[Modifier],
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError>;
    fn import_dmabuf(
        &self,
        dmabuf: &Rc<DmaBuf>,
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError>;
}

pub trait BufferObject {
    fn dmabuf(&self) -> &Rc<DmaBuf>;
    fn map_read(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError>;
    fn map_write(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError>;
    fn take_initial_sync(&self) -> Option<SyncFile>;
}

pub trait MappedBuffer {
    unsafe fn data(&self) -> &[u8];
    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    fn data_ptr(&self) -> *mut u8;
    fn stride(&self) -> i32;
}

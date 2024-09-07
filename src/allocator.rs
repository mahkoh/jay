use {
    crate::{
        format::Format,
        video::{
            dmabuf::{DmaBuf, DmaBufIds},
            drm::Drm,
            Modifier,
        },
    },
    std::{error::Error, rc::Rc},
    thiserror::Error,
};

#[derive(Debug, Error)]
#[error(transparent)]
pub struct AllocatorError(#[from] pub Box<dyn Error + Send>);

bitflags! {
    BufferUsage: u32;
        BO_USE_SCANOUT = 1 << 0,
        BO_USE_CURSOR = 1 << 1,
        BO_USE_RENDERING = 1 << 2,
        BO_USE_WRITE = 1 << 3,
        BO_USE_LINEAR = 1 << 4,
        BO_USE_PROTECTED = 1 << 5,
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
        dmabuf: &DmaBuf,
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError>;
}

pub trait BufferObject {
    fn dmabuf(&self) -> &DmaBuf;
    fn map_read(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError>;
    fn map_write(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError>;
}

pub trait MappedBuffer {
    unsafe fn data(&self) -> &[u8];
    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    fn data_ptr(&self) -> *mut u8;
    fn stride(&self) -> i32;
}

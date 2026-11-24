use crate::allocator::Allocator;
use crate::allocator::AllocatorError;
use crate::allocator::BufferObject;
use crate::allocator::BufferUsage;
use crate::format::Format;
use crate::gfx_api::SyncFile;
use crate::utils::clonecell::CloneCell;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::once::Once;
use crate::video::LINEAR_MODIFIER;
use crate::video::Modifier;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use dmabuf_allocator::udmabuf;
use dmabuf_allocator::udmabuf::Memfd;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt2;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;
use uapi::c::O_RDONLY;
use uapi::open;

#[derive(Debug, Error)]
pub enum UdmabufError {
    #[error("Could not open /dev/udmabuf")]
    Open(#[source] OsError),
    #[error("Only the linear modifier can be allocated")]
    Modifier,
    #[error("Could not create a dmabuf")]
    CreateUdmabuf(#[source] dmabuf_allocator::Error),
    #[error("Could not compute the udmabuf layout")]
    ComputeLayout(#[source] dmabuf_allocator::Error),
    #[error("Could not create a memfd")]
    CreateMemfd(#[source] dmabuf_allocator::Error),
}

#[derive(Default)]
pub struct UdmabufHolder {
    logged: Once,
    udmabuf: CloneCell<Option<Option<Rc<Udmabuf>>>>,
}

impl UdmabufHolder {
    pub fn get(&self) -> Option<Rc<Udmabuf>> {
        if let Some(u) = self.udmabuf.get() {
            return u;
        }
        match Udmabuf::new() {
            Ok(u) => {
                log::info!("Opened /dev/udmabuf");
                let u = Rc::new(u);
                self.udmabuf.set(Some(Some(u.clone())));
                Some(u)
            }
            Err(e) => {
                self.logged.exec(|| {
                    log::warn!("Unable to open /dev/udmabuf: {}", ErrorFmt(&e));
                });
                if not_matches!(e, UdmabufError::Open(OsError(c::EPERM))) {
                    self.udmabuf.set(Some(None));
                }
                None
            }
        }
    }
}

pub struct Udmabuf {
    pub fd: OwnedFd,
}

impl Udmabuf {
    pub fn new() -> Result<Self, UdmabufError> {
        let fd = open("/dev/udmabuf", O_RDONLY, 0).map_os_err(UdmabufError::Open)?;
        Ok(Self { fd })
    }

    pub fn create_dmabuf_from_memfd(
        &self,
        memfd: &OwnedFd,
        offset: usize,
        size: usize,
    ) -> Result<OwnedFd, UdmabufError> {
        udmabuf::create_dmabuf(self.fd.raw(), memfd.raw(), offset as _, size as _)
            .map(|v| v.unwrap())
            .map(OwnedFd::new)
            .map_err(UdmabufError::CreateUdmabuf)
    }

    pub fn create_dmabuf(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<Rc<DmaBuf>, UdmabufError> {
        let layout = udmabuf::layout(format.da_format, width as u32, height as u32)
            .map_err(UdmabufError::ComputeLayout)?;
        let Memfd { fd, size, .. } =
            udmabuf::create_memfd(&layout).map_err(UdmabufError::CreateMemfd)?;
        let fd = udmabuf::create_dmabuf(self.fd.raw(), fd.fd(), 0, size)
            .map(|v| OwnedFd::new(v.unwrap()))
            .map(Rc::new)
            .map_err(UdmabufError::CreateUdmabuf)?;
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

impl Allocator for Udmabuf {
    fn create_bo(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: &[Modifier],
        _usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError> {
        if !modifiers.contains(&LINEAR_MODIFIER) {
            return Err(UdmabufError::Modifier.into());
        }
        let buf = self.create_dmabuf(dma_buf_ids, width, height, format)?;
        Ok(Rc::new(UdmabufBo { buf }))
    }
}

pub struct UdmabufBo {
    buf: Rc<DmaBuf>,
}

impl BufferObject for UdmabufBo {
    fn dmabuf(&self) -> &Rc<DmaBuf> {
        &self.buf
    }

    fn take_initial_sync(&self) -> Option<SyncFile> {
        None
    }
}

impl From<UdmabufError> for AllocatorError {
    fn from(value: UdmabufError) -> Self {
        Self(Box::new(value))
    }
}

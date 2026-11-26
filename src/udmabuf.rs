use crate::format::Format;
use crate::utils::clonecell::CloneCell;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::once::Once;
use crate::video::LINEAR_MODIFIER;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use arrayvec::ArrayVec;
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

    pub fn create_dmabuf<const N: usize>(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<[Rc<DmaBuf>; N], UdmabufError> {
        let layout = udmabuf::layout(format.da_format, width as u32, height as u32)
            .map_err(UdmabufError::ComputeLayout)?;
        let Memfd { fd, size, .. } =
            udmabuf::create_memfd(&layout).map_err(UdmabufError::CreateMemfd)?;
        let mut res = ArrayVec::<_, N>::new();
        for _ in 0..N {
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
            let dmabuf = DmaBuf::new(dma_buf_ids, width, height, format, LINEAR_MODIFIER, planes);
            res.push(dmabuf);
        }
        Ok(res.into_inner().unwrap())
    }
}

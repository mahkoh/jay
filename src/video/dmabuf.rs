use {
    crate::{format::Format, utils::oserror::OsError, video::Modifier},
    arrayvec::ArrayVec,
    std::rc::Rc,
    uapi::{c::ioctl, OwnedFd, _IOW, _IOWR},
};

#[derive(Clone, Debug)]
pub struct DmaBufPlane {
    pub offset: u32,
    pub stride: u32,
    pub fd: Rc<OwnedFd>,
}

linear_ids!(DmaBufIds, DmaBufId);

#[derive(Debug, Clone)]
pub struct DmaBuf {
    pub id: DmaBufId,
    pub width: i32,
    pub height: i32,
    pub format: &'static Format,
    pub modifier: Modifier,
    pub planes: PlaneVec<DmaBufPlane>,
}

pub const MAX_PLANES: usize = 4;

pub type PlaneVec<T> = ArrayVec<T, MAX_PLANES>;

impl DmaBuf {
    pub fn is_disjoint(&self) -> bool {
        if self.planes.len() <= 1 {
            return false;
        }
        let stat = match uapi::fstat(self.planes[0].fd.raw()) {
            Ok(s) => s,
            _ => return true,
        };
        for plane in &self.planes[1..] {
            let stat2 = match uapi::fstat(plane.fd.raw()) {
                Ok(s) => s,
                _ => return true,
            };
            if stat2.st_ino != stat.st_ino {
                return true;
            }
        }
        false
    }

    pub fn import_sync_file(&self, flags: u32, sync_file: &OwnedFd) -> Result<(), OsError> {
        for plane in &self.planes {
            dma_buf_import_sync_file(&plane.fd, flags, sync_file)?;
        }
        Ok(())
    }
}

const DMA_BUF_BASE: u64 = b'b' as _;

#[allow(non_camel_case_types)]
#[repr(C)]
struct dma_buf_export_sync_file {
    flags: u32,
    fd: i32,
}

#[allow(non_camel_case_types)]
#[repr(C)]
struct dma_buf_import_sync_file {
    flags: u32,
    fd: i32,
}

pub const DMA_BUF_SYNC_READ: u32 = 1 << 0;
pub const DMA_BUF_SYNC_WRITE: u32 = 1 << 1;

const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: u64 = _IOWR::<dma_buf_export_sync_file>(DMA_BUF_BASE, 2);
const DMA_BUF_IOCTL_IMPORT_SYNC_FILE: u64 = _IOW::<dma_buf_import_sync_file>(DMA_BUF_BASE, 3);

pub fn dma_buf_export_sync_file(dmabuf: &OwnedFd, flags: u32) -> Result<OwnedFd, OsError> {
    let mut data = dma_buf_export_sync_file { flags, fd: -1 };
    let res = unsafe { ioctl(dmabuf.raw(), DMA_BUF_IOCTL_EXPORT_SYNC_FILE, &mut data) };
    if res != 0 {
        Err(OsError::default())
    } else {
        Ok(OwnedFd::new(data.fd))
    }
}

pub fn dma_buf_import_sync_file(
    dmabuf: &OwnedFd,
    flags: u32,
    sync_file: &OwnedFd,
) -> Result<(), OsError> {
    let mut data = dma_buf_import_sync_file {
        flags,
        fd: sync_file.raw(),
    };
    let res = unsafe { ioctl(dmabuf.raw(), DMA_BUF_IOCTL_IMPORT_SYNC_FILE, &mut data) };
    if res != 0 {
        Err(OsError::default())
    } else {
        Ok(())
    }
}

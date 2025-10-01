use {
    crate::{
        format::Format,
        utils::{compat::IoctlNumber, oserror::OsError},
        video::{LINEAR_MODIFIER, Modifier},
    },
    arrayvec::ArrayVec,
    std::{cell::OnceCell, rc::Rc, sync::OnceLock},
    uapi::{
        _IOW, _IOWR, OwnedFd,
        c::{self, dev_t, ioctl},
        format_ustr,
    },
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
    pub is_disjoint: OnceCell<bool>,
}

pub const MAX_PLANES: usize = 4;

pub type PlaneVec<T> = ArrayVec<T, MAX_PLANES>;

impl DmaBuf {
    pub fn is_disjoint(&self) -> bool {
        *self.is_disjoint.get_or_init(|| {
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
        })
    }

    pub fn is_one_file(&self) -> bool {
        !self.is_disjoint()
    }

    pub fn udmabuf_size(&self) -> Option<usize> {
        if self.planes.len() != 1 {
            return None;
        }
        if self.modifier != LINEAR_MODIFIER {
            return None;
        }
        let stat = match uapi::fstat(self.planes[0].fd.raw()) {
            Ok(s) => s,
            _ => return None,
        };
        static DMABUF_DEV: OnceLock<dev_t> = OnceLock::new();
        match DMABUF_DEV.get() {
            Some(d) => {
                if stat.st_dev != *d {
                    return None;
                }
            }
            None => {
                if dma_buf_export_sync_file(&self.planes[0].fd, DMA_BUF_SYNC_READ).is_err() {
                    return None;
                }
                let _ = DMABUF_DEV.set(stat.st_dev);
            }
        }
        let path = format_ustr!("/sys/kernel/dmabuf/buffers/{}/exporter_name", stat.st_ino);
        let Ok(file) = uapi::open(path, c::O_RDONLY, 0) else {
            return None;
        };
        const MARKER: &[u8] = b"udmabuf\n";
        let mut buf = [0u8; MARKER.len()];
        if uapi::read(file.raw(), &mut buf).is_err() {
            return None;
        }
        if buf != MARKER {
            return None;
        }
        Some(stat.st_size as usize)
    }

    pub fn import_sync_file(&self, flags: u32, sync_file: &OwnedFd) -> Result<(), OsError> {
        for plane in &self.planes {
            dma_buf_import_sync_file(&plane.fd, flags, sync_file)?;
            if self.is_one_file() {
                break;
            }
        }
        Ok(())
    }
}

const DMA_BUF_BASE: u64 = b'b' as _;

#[repr(C)]
struct dma_buf_export_sync_file {
    flags: u32,
    fd: i32,
}

#[repr(C)]
struct dma_buf_import_sync_file {
    flags: u32,
    fd: i32,
}

pub const DMA_BUF_SYNC_READ: u32 = 1 << 0;
pub const DMA_BUF_SYNC_WRITE: u32 = 1 << 1;

const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: IoctlNumber =
    _IOWR::<dma_buf_export_sync_file>(DMA_BUF_BASE, 2) as IoctlNumber;
const DMA_BUF_IOCTL_IMPORT_SYNC_FILE: IoctlNumber =
    _IOW::<dma_buf_import_sync_file>(DMA_BUF_BASE, 3) as IoctlNumber;

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

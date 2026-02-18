use {
    crate::{
        allocator::{Allocator, AllocatorError, BufferObject, BufferUsage, MappedBuffer},
        format::Format,
        utils::{
            clonecell::CloneCell, compat::IoctlNumber, errorfmt::ErrorFmt, once::Once,
            oserror::OsError, page_size::page_size,
        },
        video::{
            LINEAR_MODIFIER, LINEAR_STRIDE_ALIGN, Modifier,
            dmabuf::{DmaBuf, DmaBufIds, DmaBufPlane, PlaneVec},
            drm::Drm,
        },
    },
    std::{ptr, rc::Rc},
    thiserror::Error,
    uapi::{
        _IOW, OwnedFd,
        c::{
            self, F_SEAL_SHRINK, MAP_SHARED, MFD_ALLOW_SEALING, O_RDONLY, PROT_READ, PROT_WRITE,
            ioctl, mmap, munmap,
        },
        map_err, open,
    },
};

#[derive(Debug, Error)]
pub enum UdmabufError {
    #[error("Could not open /dev/udmabuf")]
    Open(#[source] OsError),
    #[error("Only the linear modifier can be allocated")]
    Modifier,
    #[error("Could not create a memfd")]
    Memfd(#[source] OsError),
    #[error("Size calculation overflowed")]
    Overflow,
    #[error("Could not resize the memfd")]
    Truncate(#[source] OsError),
    #[error("Could not seal the memfd")]
    Seal(#[source] OsError),
    #[error("Could not create a dmabuf")]
    CreateDmabuf(#[source] OsError),
    #[error("Only a single plane is supported")]
    Planes,
    #[error("Stride is invalid")]
    Stride,
    #[error("Could not stat the dmabuf")]
    Stat(#[source] OsError),
    #[error("Dmabuf is too small for required size")]
    Size,
    #[error("Could not map dmabuf")]
    Map(#[source] OsError),
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
    fd: OwnedFd,
}

impl Udmabuf {
    pub fn new() -> Result<Self, UdmabufError> {
        let fd = match open("/dev/udmabuf", O_RDONLY, 0) {
            Ok(b) => b,
            Err(e) => return Err(UdmabufError::Open(e.into())),
        };
        Ok(Self { fd })
    }

    pub fn create_dmabuf_from_memfd(
        &self,
        memfd: &OwnedFd,
        offset: usize,
        size: usize,
    ) -> Result<OwnedFd, UdmabufError> {
        let mut cmd = udmabuf_create {
            memfd: memfd.raw() as u32,
            flags: UDMABUF_FLAGS_CLOEXEC,
            offset: offset as u64,
            size: size as u64,
        };
        let dmabuf = unsafe { ioctl(self.fd.raw(), UDMABUF_CREATE, &mut cmd) };
        let dmabuf = match map_err!(dmabuf) {
            Ok(d) => OwnedFd::new(d),
            Err(e) => return Err(UdmabufError::CreateDmabuf(e.into())),
        };
        Ok(dmabuf)
    }

    pub fn create_dmabuf(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<DmaBuf, UdmabufError> {
        Ok(self.create_bo(dma_buf_ids, width, height, format)?.buf)
    }

    fn create_bo(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<UdmabufBo, UdmabufError> {
        let height = height as u64;
        let width = width as u64;
        if height > 1 << 16 || width > 1 << 16 {
            return Err(UdmabufError::Overflow);
        }
        let stride = (width * format.bpp as u64).next_multiple_of(LINEAR_STRIDE_ALIGN);
        let size_mask = page_size() as u64 - 1;
        let size = (height * stride + size_mask) & !size_mask;
        let memfd = match uapi::memfd_create("udmabuf", MFD_ALLOW_SEALING) {
            Ok(f) => f,
            Err(e) => return Err(UdmabufError::Memfd(e.into())),
        };
        if let Err(e) = uapi::ftruncate(memfd.raw(), size as _) {
            return Err(UdmabufError::Truncate(e.into()));
        }
        if let Err(e) = uapi::fcntl_add_seals(memfd.raw(), F_SEAL_SHRINK) {
            return Err(UdmabufError::Seal(e.into()));
        }
        let dmabuf = self.create_dmabuf_from_memfd(&memfd, 0, size as _)?;
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: 0,
            stride: stride as _,
            fd: Rc::new(dmabuf),
        });
        let dmabuf = DmaBuf {
            id: dma_buf_ids.next(),
            width: width as _,
            height: height as _,
            format,
            modifier: LINEAR_MODIFIER,
            planes,
            is_disjoint: Default::default(),
        };
        Ok(UdmabufBo {
            buf: dmabuf,
            size: size as _,
        })
    }
}

impl Allocator for Udmabuf {
    fn drm(&self) -> Option<&Drm> {
        None
    }

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
        Ok(Rc::new(self.create_bo(
            dma_buf_ids,
            width,
            height,
            format,
        )?))
    }

    fn import_dmabuf(
        &self,
        dmabuf: &DmaBuf,
        _usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError> {
        if dmabuf.planes.len() != 1 {
            return Err(UdmabufError::Planes.into());
        }
        if dmabuf.modifier != LINEAR_MODIFIER {
            return Err(UdmabufError::Modifier.into());
        }
        let plane = &dmabuf.planes[0];
        let height = dmabuf.height as u64;
        let width = dmabuf.width as u64;
        let stride = plane.stride as u64;
        let offset = plane.offset as u64;
        if height > 1 << 16 || width > 1 << 16 {
            return Err(UdmabufError::Overflow.into());
        }
        if stride < width * dmabuf.format.bpp as u64 {
            return Err(UdmabufError::Stride.into());
        }
        let size = offset + stride * height;
        if usize::try_from(size).is_err() {
            return Err(UdmabufError::Overflow.into());
        }
        let stat = match uapi::fstat(plane.fd.raw()) {
            Ok(s) => s,
            Err(e) => return Err(UdmabufError::Stat(e.into()).into()),
        };
        if (stat.st_size as u64) < size {
            return Err(UdmabufError::Size.into());
        }
        Ok(Rc::new(UdmabufBo {
            buf: dmabuf.clone(),
            size: size as usize,
        }))
    }
}

struct UdmabufBo {
    buf: DmaBuf,
    size: usize,
}

impl BufferObject for UdmabufBo {
    fn dmabuf(&self) -> &DmaBuf {
        &self.buf
    }

    fn map_read(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError> {
        self.map_write()
    }

    fn map_write(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError> {
        let plane = &self.buf.planes[0];
        unsafe {
            let res = mmap(
                ptr::null_mut(),
                self.size,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                plane.fd.raw(),
                0,
            );
            if res == c::MAP_FAILED {
                return Err(UdmabufError::Map(OsError::default()).into());
            }
            let offset = plane.offset as _;
            let data =
                std::slice::from_raw_parts_mut((res as *mut u8).add(offset), self.size - offset);
            Ok(Box::new(UdmabufMap {
                data,
                stride: plane.stride as _,
                ptr: res,
                len: self.size,
                _bo: self,
            }))
        }
    }
}

struct UdmabufMap {
    _bo: Rc<UdmabufBo>,
    data: *mut [u8],
    stride: i32,
    ptr: *mut c::c_void,
    len: usize,
}

impl Drop for UdmabufMap {
    fn drop(&mut self) {
        unsafe {
            let res = munmap(self.ptr, self.len);
            if let Err(e) = map_err!(res) {
                log::error!("Could not unmap udmabuf: {}", OsError::from(e));
            }
        }
    }
}

impl MappedBuffer for UdmabufMap {
    unsafe fn data(&self) -> &[u8] {
        unsafe { &*self.data }
    }

    fn data_ptr(&self) -> *mut u8 {
        self.data as _
    }

    fn stride(&self) -> i32 {
        self.stride
    }
}

impl From<UdmabufError> for AllocatorError {
    fn from(value: UdmabufError) -> Self {
        Self(Box::new(value))
    }
}

#[repr(C)]
#[derive(Debug)]
struct udmabuf_create {
    memfd: u32,
    flags: u32,
    offset: u64,
    size: u64,
}

const UDMABUF_FLAGS_CLOEXEC: u32 = 0x01;

const UDMABUF_CREATE: IoctlNumber = _IOW::<udmabuf_create>(b'u' as u64, 0x42) as IoctlNumber;

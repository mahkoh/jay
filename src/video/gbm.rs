#![allow(non_camel_case_types)]

use {
    crate::{
        allocator::{
            Allocator, AllocatorError, BufferObject, BufferUsage, MappedBuffer, BO_USE_CURSOR,
            BO_USE_LINEAR, BO_USE_PROTECTED, BO_USE_RENDERING, BO_USE_SCANOUT, BO_USE_WRITE,
        },
        format::{formats, Format},
        utils::oserror::OsError,
        video::{
            dmabuf::{DmaBuf, DmaBufIds, DmaBufPlane, PlaneVec},
            drm::{Drm, DrmError},
            Modifier, INVALID_MODIFIER,
        },
    },
    std::{
        fmt::{Debug, Formatter},
        ptr,
        rc::Rc,
        slice,
    },
    thiserror::Error,
    uapi::{c, OwnedFd},
};

#[derive(Debug, Error)]
pub enum GbmError {
    #[error("The drm subsystem returned an error")]
    Drm(#[from] DrmError),
    #[error("Cloud not create a gbm device")]
    CreateDevice,
    #[error("Cloud not create a gbm buffer")]
    CreateBo(#[source] OsError),
    #[error("gbm buffer has an unknown format")]
    UnknownFormat,
    #[error("Could not retrieve a drm-buf fd")]
    DrmFd,
    #[error("Could not map bo")]
    MapBo(#[source] OsError),
    #[error("Tried to allocate a buffer with no modifier")]
    NoModifier,
}

impl From<GbmError> for AllocatorError {
    fn from(value: GbmError) -> Self {
        Self(Box::new(value))
    }
}

pub type Device = u8;
type Bo = u8;

pub const GBM_BO_USE_SCANOUT: u32 = 1 << 0;
pub const GBM_BO_USE_CURSOR: u32 = 1 << 1;
pub const GBM_BO_USE_RENDERING: u32 = 1 << 2;
pub const GBM_BO_USE_WRITE: u32 = 1 << 3;
pub const GBM_BO_USE_LINEAR: u32 = 1 << 4;
pub const GBM_BO_USE_PROTECTED: u32 = 1 << 5;

#[expect(dead_code)]
const GBM_BO_IMPORT_WL_BUFFER: u32 = 0x5501;
#[expect(dead_code)]
const GBM_BO_IMPORT_EGL_IMAGE: u32 = 0x5502;
#[expect(dead_code)]
const GBM_BO_IMPORT_FD: u32 = 0x5503;
const GBM_BO_IMPORT_FD_MODIFIER: u32 = 0x5504;

const GBM_BO_TRANSFER_READ: u32 = 1 << 0;
const GBM_BO_TRANSFER_WRITE: u32 = 1 << 1;
const GBM_BO_TRANSFER_READ_WRITE: u32 = GBM_BO_TRANSFER_READ | GBM_BO_TRANSFER_WRITE;

#[repr(C)]
struct gbm_import_fd_modifier_data {
    width: u32,
    height: u32,
    format: u32,
    num_fds: u32,
    fds: [c::c_int; 4],
    strides: [c::c_int; 4],
    offsets: [c::c_int; 4],
    modifier: u64,
}

#[link(name = "gbm")]
unsafe extern "C" {
    fn gbm_create_device(fd: c::c_int) -> *mut Device;
    fn gbm_device_destroy(dev: *mut Device);

    fn gbm_bo_import(dev: *mut Device, ty: u32, buffer: *mut u8, flags: u32) -> *mut Bo;
    fn gbm_bo_create_with_modifiers2(
        dev: *mut Device,
        width: u32,
        height: u32,
        format: u32,
        modifiers: *const u64,
        count: c::c_uint,
        flags: u32,
    ) -> *mut Bo;
    fn gbm_bo_destroy(bo: *mut Bo);
    fn gbm_bo_get_plane_count(bo: *mut Bo) -> c::c_int;
    fn gbm_bo_get_width(bo: *mut Bo) -> u32;
    fn gbm_bo_get_height(bo: *mut Bo) -> u32;
    #[expect(dead_code)]
    fn gbm_bo_get_stride(bo: *mut Bo) -> u32;
    fn gbm_bo_get_modifier(bo: *mut Bo) -> u64;
    fn gbm_bo_get_stride_for_plane(bo: *mut Bo, plane: c::c_int) -> u32;
    fn gbm_bo_get_fd_for_plane(bo: *mut Bo, plane: c::c_int) -> c::c_int;
    fn gbm_bo_get_offset(bo: *mut Bo, plane: c::c_int) -> u32;
    fn gbm_bo_get_format(bo: *mut Bo) -> u32;
    #[expect(dead_code)]
    fn gbm_bo_get_bpp(bo: *mut Bo) -> u32;
    fn gbm_bo_map(
        bo: *mut Bo,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        flags: u32,
        strid: *mut u32,
        map_data: *mut *mut u8,
    ) -> *mut u8;
    fn gbm_bo_unmap(bo: *mut Bo, map_data: *mut u8);
}

pub struct GbmDevice {
    pub drm: Drm,
    dev: *mut Device,
}

impl Debug for GbmDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbmDevice").finish_non_exhaustive()
    }
}

struct BoHolder {
    bo: *mut Bo,
}

pub struct GbmBo {
    bo: BoHolder,
    dmabuf: DmaBuf,
}

impl Debug for GbmBo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbmBo").finish_non_exhaustive()
    }
}

pub struct GbmBoMap {
    bo: Rc<GbmBo>,
    data: *mut [u8],
    opaque: *mut u8,
    stride: i32,
}

impl MappedBuffer for GbmBoMap {
    unsafe fn data(&self) -> &[u8] {
        &*self.data
    }

    fn data_ptr(&self) -> *mut u8 {
        self.data as _
    }

    fn stride(&self) -> i32 {
        self.stride
    }
}

unsafe fn export_bo(dmabuf_ids: &DmaBufIds, bo: *mut Bo) -> Result<DmaBuf, GbmError> {
    Ok(DmaBuf {
        id: dmabuf_ids.next(),
        width: gbm_bo_get_width(bo) as _,
        height: gbm_bo_get_height(bo) as _,
        modifier: gbm_bo_get_modifier(bo),
        format: {
            let format = gbm_bo_get_format(bo);
            match formats().get(&format).copied() {
                Some(f) => f,
                _ => return Err(GbmError::UnknownFormat),
            }
        },
        planes: {
            let mut planes = PlaneVec::new();
            for plane in 0..gbm_bo_get_plane_count(bo) {
                let offset = gbm_bo_get_offset(bo, plane);
                let stride = gbm_bo_get_stride_for_plane(bo, plane);
                let fd = gbm_bo_get_fd_for_plane(bo, plane);
                if fd < 0 {
                    return Err(GbmError::DrmFd);
                }
                planes.push(DmaBufPlane {
                    offset,
                    stride,
                    fd: Rc::new(OwnedFd::new(fd)),
                })
            }
            planes
        },
    })
}

impl GbmDevice {
    pub fn new(drm: &Drm) -> Result<Self, GbmError> {
        let drm = drm.dup_render()?;
        let dev = unsafe { gbm_create_device(drm.raw()) };
        if dev.is_null() {
            Err(GbmError::CreateDevice)
        } else {
            Ok(Self { drm, dev })
        }
    }

    pub fn raw(&self) -> *mut Device {
        self.dev
    }

    pub fn create_bo<'a>(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &Format,
        modifiers: impl IntoIterator<Item = &'a Modifier>,
        mut usage: u32,
    ) -> Result<GbmBo, GbmError> {
        unsafe {
            let modifiers: Vec<Modifier> = modifiers.into_iter().copied().collect();
            if modifiers.is_empty() {
                return Err(GbmError::NoModifier);
            }
            let (modifiers, n_modifiers) = if modifiers == [INVALID_MODIFIER] {
                (ptr::null(), 0)
            } else {
                usage &= !GBM_BO_USE_LINEAR;
                (modifiers.as_ptr() as _, modifiers.len() as _)
            };
            let bo = gbm_bo_create_with_modifiers2(
                self.dev,
                width as _,
                height as _,
                format.drm,
                modifiers,
                n_modifiers,
                usage,
            );
            if bo.is_null() {
                return Err(GbmError::CreateBo(OsError::default()));
            }
            let bo = BoHolder { bo };
            let mut dma = export_bo(dma_buf_ids, bo.bo)?;
            if modifiers.is_null() {
                dma.modifier = INVALID_MODIFIER;
            }
            Ok(GbmBo { bo, dmabuf: dma })
        }
    }

    pub fn import_dmabuf(&self, dmabuf: &DmaBuf, usage: u32) -> Result<GbmBo, GbmError> {
        let mut import = gbm_import_fd_modifier_data {
            width: dmabuf.width as _,
            height: dmabuf.height as _,
            format: dmabuf.format.drm as _,
            num_fds: dmabuf.planes.len() as _,
            fds: [0; 4],
            strides: [0; 4],
            offsets: [0; 4],
            modifier: dmabuf.modifier,
        };
        for (i, plane) in dmabuf.planes.iter().enumerate() {
            import.fds[i] = plane.fd.raw();
            import.strides[i] = plane.stride as _;
            import.offsets[i] = plane.offset as _;
        }
        unsafe {
            let bo = gbm_bo_import(
                self.dev,
                GBM_BO_IMPORT_FD_MODIFIER,
                &mut import as *const _ as _,
                usage,
            );
            if bo.is_null() {
                return Err(GbmError::CreateBo(OsError::default()));
            }
            let bo = BoHolder { bo };
            Ok(GbmBo {
                bo,
                dmabuf: dmabuf.clone(),
            })
        }
    }
}

impl Allocator for GbmDevice {
    fn drm(&self) -> Option<&Drm> {
        Some(&self.drm)
    }

    fn create_bo(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: &[Modifier],
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError> {
        let usage = map_usage(usage);
        self.create_bo(dma_buf_ids, width, height, format, modifiers, usage)
            .map(|v| Rc::new(v) as _)
            .map_err(|v| v.into())
    }

    fn import_dmabuf(
        &self,
        dmabuf: &DmaBuf,
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError> {
        let usage = map_usage(usage);
        self.import_dmabuf(dmabuf, usage)
            .map(|v| Rc::new(v) as _)
            .map_err(|v| v.into())
    }
}

fn map_usage(usage: BufferUsage) -> u32 {
    let mut gbm = 0;
    macro_rules! map {
        ($bu:ident to $gbu:ident) => {
            if usage.contains($bu) {
                gbm |= $gbu;
            }
        };
    }
    map!(BO_USE_SCANOUT to GBM_BO_USE_SCANOUT);
    map!(BO_USE_CURSOR to GBM_BO_USE_CURSOR);
    map!(BO_USE_RENDERING to GBM_BO_USE_RENDERING);
    map!(BO_USE_WRITE to GBM_BO_USE_WRITE);
    map!(BO_USE_LINEAR to GBM_BO_USE_LINEAR);
    map!(BO_USE_PROTECTED to GBM_BO_USE_PROTECTED);
    gbm
}

impl Drop for GbmDevice {
    fn drop(&mut self) {
        unsafe {
            gbm_device_destroy(self.dev);
        }
    }
}

impl GbmBo {
    pub fn map_read(self: &Rc<Self>) -> Result<GbmBoMap, GbmError> {
        self.map2(GBM_BO_TRANSFER_READ)
    }

    pub fn map_write(self: &Rc<Self>) -> Result<GbmBoMap, GbmError> {
        self.map2(GBM_BO_TRANSFER_READ_WRITE)
    }

    fn map2(self: &Rc<Self>, flags: u32) -> Result<GbmBoMap, GbmError> {
        let mut stride = 0;
        let mut map_data = ptr::null_mut();
        unsafe {
            let map = gbm_bo_map(
                self.bo.bo,
                0,
                0,
                self.dmabuf.width as _,
                self.dmabuf.height as _,
                flags,
                &mut stride,
                &mut map_data,
            );
            if map.is_null() {
                return Err(GbmError::MapBo(OsError::default()));
            }
            let map = slice::from_raw_parts_mut(map, (stride * self.dmabuf.height as u32) as usize);
            Ok(GbmBoMap {
                bo: self.clone(),
                data: map,
                opaque: map_data,
                stride: stride as i32,
            })
        }
    }
}

impl BufferObject for GbmBo {
    fn dmabuf(&self) -> &DmaBuf {
        &self.dmabuf
    }

    fn map_read(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError> {
        GbmBo::map_read(&self)
            .map(|v| Box::new(v) as _)
            .map_err(|v| v.into())
    }

    fn map_write(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError> {
        GbmBo::map_write(&self)
            .map(|v| Box::new(v) as _)
            .map_err(|v| v.into())
    }
}

impl Drop for GbmBoMap {
    fn drop(&mut self) {
        unsafe {
            gbm_bo_unmap(self.bo.bo.bo, self.opaque);
        }
    }
}

impl Drop for BoHolder {
    fn drop(&mut self) {
        unsafe {
            gbm_bo_destroy(self.bo);
        }
    }
}

#![allow(non_camel_case_types)]

use {
    crate::{
        format::{formats, Format},
        utils::oserror::OsError,
        video::{
            dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
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

pub type Device = u8;
type Bo = u8;

pub const GBM_BO_USE_SCANOUT: u32 = 1 << 0;
#[allow(dead_code)]
pub const GBM_BO_USE_CURSOR: u32 = 1 << 1;
pub const GBM_BO_USE_RENDERING: u32 = 1 << 2;
#[allow(dead_code)]
pub const GBM_BO_USE_WRITE: u32 = 1 << 3;
pub const GBM_BO_USE_LINEAR: u32 = 1 << 4;
#[allow(dead_code)]
pub const GBM_BO_USE_PROTECTED: u32 = 1 << 5;

#[allow(dead_code)]
const GBM_BO_IMPORT_WL_BUFFER: u32 = 0x5501;
#[allow(dead_code)]
const GBM_BO_IMPORT_EGL_IMAGE: u32 = 0x5502;
#[allow(dead_code)]
const GBM_BO_IMPORT_FD: u32 = 0x5503;
const GBM_BO_IMPORT_FD_MODIFIER: u32 = 0x5504;

const GBM_BO_TRANSFER_READ: u32 = 1 << 0;
#[allow(dead_code)]
const GBM_BO_TRANSFER_WRITE: u32 = 1 << 1;
#[allow(dead_code)]
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
extern "C" {
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
    #[allow(dead_code)]
    fn gbm_bo_get_stride(bo: *mut Bo) -> u32;
    fn gbm_bo_get_modifier(bo: *mut Bo) -> u64;
    fn gbm_bo_get_stride_for_plane(bo: *mut Bo, plane: c::c_int) -> u32;
    fn gbm_bo_get_fd_for_plane(bo: *mut Bo, plane: c::c_int) -> c::c_int;
    fn gbm_bo_get_offset(bo: *mut Bo, plane: c::c_int) -> u32;
    fn gbm_bo_get_format(bo: *mut Bo) -> u32;
    #[allow(dead_code)]
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

pub struct GbmBoMap {
    bo: Rc<GbmBo>,
    data: *mut [u8],
    opaque: *mut u8,
}

impl GbmBoMap {
    pub unsafe fn data(&self) -> &[u8] {
        &*self.data
    }
}

unsafe fn export_bo(bo: *mut Bo) -> Result<DmaBuf, GbmError> {
    Ok(DmaBuf {
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
            log::info!("modifiers: {:?}", modifiers);
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
            let dma = export_bo(bo.bo)?;
            log::info!("modifier {:?}", dma.modifier);
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

impl Drop for GbmDevice {
    fn drop(&mut self) {
        unsafe {
            gbm_device_destroy(self.dev);
        }
    }
}

impl GbmBo {
    pub fn dmabuf(&self) -> &DmaBuf {
        &self.dmabuf
    }

    pub fn map(self: &Rc<Self>) -> Result<GbmBoMap, GbmError> {
        let mut stride = 0;
        let mut map_data = ptr::null_mut();
        unsafe {
            let map = gbm_bo_map(
                self.bo.bo,
                0,
                0,
                self.dmabuf.width as _,
                self.dmabuf.height as _,
                GBM_BO_TRANSFER_READ,
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
            })
        }
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

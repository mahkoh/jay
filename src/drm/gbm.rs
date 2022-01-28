use crate::drm::dma::{DmaBuf, DmaBufPlane};
use crate::drm::drm::{Drm, DrmError};
use crate::drm::{ModifiedFormat, INVALID_MODIFIER};
use crate::format::formats;
use std::ptr;
use thiserror::Error;
use uapi::{c, OwnedFd};

#[derive(Debug, Error)]
pub enum GbmError {
    #[error("The drm subsystem returned an error")]
    Drm(#[from] DrmError),
    #[error("Cloud not create a gbm device")]
    CreateDevice,
    #[error("Cloud not create a gbm buffer")]
    CreateBo,
    #[error("gbm buffer has an unknown format")]
    UnknownFormat,
    #[error("Could not retrieve a drm-buf fd")]
    DrmFd,
}

type Device = u8;
type Bo = u8;

pub const GBM_BO_USE_SCANOUT: u32 = 1 << 0;
pub const GBM_BO_USE_CURSOR: u32 = 1 << 1;
pub const GBM_BO_USE_RENDERING: u32 = 1 << 2;
pub const GBM_BO_USE_WRITE: u32 = 1 << 3;
pub const GBM_BO_USE_LINEAR: u32 = 1 << 4;
pub const GBM_BO_USE_PROTECTED: u32 = 1 << 5;

#[link(name = "gbm")]
extern "C" {
    fn gbm_create_device(fd: c::c_int) -> *mut Device;
    fn gbm_device_destroy(dev: *mut Device);

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
    fn gbm_bo_get_stride(bo: *mut Bo) -> u32;
    fn gbm_bo_get_modifier(bo: *mut Bo) -> u64;
    fn gbm_bo_get_stride_for_plane(bo: *mut Bo, plane: c::c_int) -> u32;
    fn gbm_bo_get_fd_for_plane(bo: *mut Bo, plane: c::c_int) -> c::c_int;
    fn gbm_bo_get_offset(bo: *mut Bo, plane: c::c_int) -> u32;
    fn gbm_bo_get_format(bo: *mut Bo) -> u32;
    fn gbm_bo_get_bpp(bo: *mut Bo) -> u32;
}

pub struct GbmDevice {
    drm: Drm,
    dev: *mut Device,
}

struct BoHolder {
    bo: *mut Bo,
}

pub struct GbmBo {
    bo: BoHolder,
    dma: DmaBuf,
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
            let mut planes = vec![];
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
                    fd: OwnedFd::new(fd),
                })
            }
            planes
        },
    })
}

impl GbmDevice {
    pub fn new(drm: &Drm) -> Result<Self, GbmError> {
        let drm = drm.dup_unprivileged()?;
        let dev = unsafe { gbm_create_device(drm.raw()) };
        if dev.is_null() {
            Err(GbmError::CreateDevice)
        } else {
            Ok(Self { drm, dev })
        }
    }

    pub fn create_bo(
        &self,
        width: i32,
        height: i32,
        format: &ModifiedFormat,
        usage: u32,
    ) -> Result<GbmBo, GbmError> {
        unsafe {
            let (modifiers, n_modifiers) = if format.modifier == INVALID_MODIFIER {
                (ptr::null(), 0)
            } else {
                (&format.modifier as _, 1)
            };
            let bo = gbm_bo_create_with_modifiers2(
                self.dev,
                width as _,
                height as _,
                format.format.drm,
                modifiers,
                n_modifiers,
                usage,
            );
            if bo.is_null() {
                return Err(GbmError::CreateBo);
            }
            let bo = BoHolder { bo };
            let dma = export_bo(bo.bo)?;
            Ok(GbmBo { bo, dma })
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
    pub fn dma(&self) -> &DmaBuf {
        &self.dma
    }
}

impl Drop for BoHolder {
    fn drop(&mut self) {
        unsafe {
            gbm_bo_destroy(self.bo);
        }
    }
}

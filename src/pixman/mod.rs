mod consts;

include!(concat!(env!("OUT_DIR"), "/pixman_tys.rs"));

use crate::ClientMemError;
pub use consts::*;
use std::cell::Cell;
use std::ptr;
use thiserror::Error;
use uapi::c;

#[link(name = "pixman-1")]
#[allow(improper_ctypes)]
extern "C" {
    fn pixman_region32_init(region: *mut Region);
    fn pixman_region32_init_rect(
        region: *mut Region,
        x: c::c_int,
        y: c::c_int,
        width: c::c_uint,
        height: c::c_uint,
    );
    fn pixman_region32_fini(region: *mut Region);
    fn pixman_region32_copy(dst: *mut Region, src: *const Region);
    fn pixman_region32_union(dst: *mut Region, a: *const Region, b: *const Region);
    fn pixman_region32_subtract(dst: *mut Region, a: *const Region, b: *const Region);
    fn pixman_image_create_bits_no_clear(
        format: PixmanFormat,
        width: c::c_int,
        height: c::c_int,
        bits: *mut u32,
        stride: c::c_int,
    ) -> *mut PixmanImage;
    fn pixman_image_unref(image: *mut PixmanImage) -> c::c_int;
    // fn pixman_image_ref(image: *mut PixmanImage) -> *mut PixmanImage;
    fn pixman_image_fill_boxes(
        op: PixmanOp,
        dest: *mut PixmanImage,
        color: *const Color,
        nboxes: c::c_int,
        boxes: *const Box32,
    ) -> c::c_int;
    fn pixman_image_composite32(
        op: PixmanOp,
        src: *mut PixmanImage,
        mask: *mut PixmanImage,
        dest: *mut PixmanImage,
        src_x: i32,
        src_y: i32,
        mask_x: i32,
        mask_y: i32,
        dest_x: i32,
        dest_y: i32,
        width: i32,
        height: i32,
    );
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Box32 {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Color {
    red: u16,
    green: u16,
    blue: u16,
    alpha: u16,
}

#[repr(C)]
struct RegionData {
    size: c::c_long,
    num_rects: c::c_long,
    // rects: [Box32; size],
}

#[repr(C)]
pub struct Region {
    extents: Box32,
    data: *mut RegionData,
}

impl Region {
    pub fn new() -> Self {
        let mut slf = Region {
            extents: Default::default(),
            data: ptr::null_mut(),
        };
        unsafe {
            pixman_region32_init(&mut slf);
        }
        slf
    }

    pub fn rect(x: i32, y: i32, width: u32, height: u32) -> Self {
        let mut new = Region::new();
        unsafe {
            pixman_region32_init_rect(&mut new, x as _, y as _, width as _, height as _);
        }
        new
    }

    pub fn add(&self, region: &Self) -> Self {
        let mut new = Region::new();
        unsafe {
            pixman_region32_union(&mut new, self, region);
        }
        new
    }

    pub fn subtract(&self, region: &Self) -> Self {
        let mut new = Region::new();
        unsafe {
            pixman_region32_subtract(&mut new, self, region);
        }
        new
    }
}

impl Clone for Region {
    fn clone(&self) -> Self {
        let mut new = Region::new();
        unsafe {
            pixman_region32_copy(&mut new, self);
        }
        new
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        unsafe {
            pixman_region32_fini(self);
        }
    }
}

pub unsafe trait PixmanMemory: Clone {
    type E;

    fn access<T, F>(&self, f: F) -> Result<T, Self::E>
    where
        F: FnOnce(&[Cell<u8>]) -> T;
}

#[derive(Debug, Error)]
pub enum PixmanError {
    #[error("The image size values cannot be represented in c_int")]
    SizeOverflow,
    #[error("The pixman memory does not contain enough memory to hold the image")]
    InsufficientMemory,
    #[error("The stride does not contain enough bytes to contain a row")]
    RowOverflow,
    #[error("Pixman images must be aligned at a 4 byte boundary")]
    UnalignedMemory,
    #[error("Internal pixman error")]
    Internal,
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(PixmanError, ClientMemError, ClientMemError);

impl From<!> for PixmanError {
    fn from(_: !) -> Self {
        unreachable!()
    }
}

struct PixmanImage;

pub struct Image<T> {
    data: *mut PixmanImage,
    width: u32,
    height: u32,
    memory: T,
}

impl<T: PixmanMemory> Image<T>
where
    PixmanError: From<<T as PixmanMemory>::E>,
{
    pub fn new(
        memory: T,
        format: Format,
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<Self, PixmanError> {
        let data = memory.access(|m| {
            if format_bpp(format.raw()) as u64 * width as u64 > stride as u64 * 8 {
                return Err(PixmanError::RowOverflow);
            }
            if (m.len() as u64) < height as u64 * stride as u64 {
                return Err(PixmanError::InsufficientMemory);
            }
            let (width, height, stride) =
                match (width.try_into(), height.try_into(), stride.try_into()) {
                    (Ok(w), Ok(h), Ok(s)) => (w, h, s),
                    _ => return Err(PixmanError::SizeOverflow),
                };
            if m.as_ptr() as usize % 4 != 0 {
                return Err(PixmanError::UnalignedMemory);
            }
            let data = unsafe {
                pixman_image_create_bits_no_clear(
                    format.raw() as _,
                    width,
                    height,
                    m.as_ptr() as _,
                    stride,
                )
            };
            if data.is_null() {
                return Err(PixmanError::Internal);
            }
            Ok(data)
        })??;
        Ok(Self {
            data,
            width,
            height,
            memory,
        })
    }

    pub fn fill(&self, r: u8, g: u8, b: u8, a: u8) -> Result<(), PixmanError> {
        self.fill_rect(r, g, b, a, 0, 0, self.width as _, self.height as _)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_rect(
        &self,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    ) -> Result<(), PixmanError> {
        self.memory.access(|_| {
            let bx = Box32 { x1, y1, x2, y2 };
            let color = Color {
                red: (r as u16) << 8,
                green: (g as u16) << 8,
                blue: (b as u16) << 8,
                alpha: (a as u16) << 8,
            };
            unsafe {
                pixman_image_fill_boxes(OP_SRC.raw() as PixmanOp, self.data, &color, 1, &bx);
            }
        })?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_insert_border(
        &self,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        width: i32,
    ) -> Result<(), PixmanError> {
        self.memory.access(|_| {
            let mut bx = [
                Box32 {
                    x1,
                    y1,
                    x2,
                    y2: y1 + width,
                },
                Box32 {
                    x1: x2 - width,
                    y1,
                    x2,
                    y2,
                },
                Box32 {
                    x1,
                    y1,
                    x2: x1 + width,
                    y2,
                },
                Box32 {
                    x1,
                    y1: y2 - width,
                    x2,
                    y2,
                },
            ];
            for bx in &mut bx {
                bx.x1 = bx.x1.max(0).min(self.width as i32);
                bx.x2 = bx.x2.max(0).min(self.width as i32);
                bx.y1 = bx.y1.max(0).min(self.height as i32);
                bx.y2 = bx.y2.max(0).min(self.height as i32);
            }
            let color = Color {
                red: (r as u16) << 8,
                green: (g as u16) << 8,
                blue: (b as u16) << 8,
                alpha: (a as u16) << 8,
            };
            unsafe {
                pixman_image_fill_boxes(
                    OP_SRC.raw() as PixmanOp,
                    self.data,
                    &color,
                    bx.len() as _,
                    bx.as_ptr(),
                );
            }
        })?;
        Ok(())
    }

    pub fn add_image<U>(&self, over: &Image<U>, x: i32, y: i32) -> Result<(), PixmanError>
    where
        U: PixmanMemory,
        PixmanError: From<<U as PixmanMemory>::E>,
    {
        self.memory.access(|_| {
            over.memory.access(|_| unsafe {
                pixman_image_composite32(
                    OP_OVER.raw(),
                    over.data,
                    ptr::null_mut(),
                    self.data,
                    0,
                    0,
                    0,
                    0,
                    x,
                    y,
                    over.width as _,
                    over.height as _,
                );
            })
        })??;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn memory(&self) -> &T {
        &self.memory
    }
}

impl<T> Drop for Image<T> {
    fn drop(&mut self) {
        unsafe {
            pixman_image_unref(self.data);
        }
    }
}

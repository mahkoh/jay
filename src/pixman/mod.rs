use std::ops::{Add, Sub};
use std::ptr;
use uapi::c;

#[link(name = "pixman-1")]
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
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Box32 {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
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

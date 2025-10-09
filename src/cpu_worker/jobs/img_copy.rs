use {
    crate::{
        cpu_worker::{AsyncCpuWork, CpuWork},
        rect::Rect,
    },
    std::{ptr, time::Instant},
};

pub struct ImgCopyWork {
    pub src: *mut u8,
    pub dst: *mut u8,
    pub width: i32,
    pub stride: i32,
    pub bpp: i32,
    pub rects: Vec<Rect>,
    _priv: (),
}

unsafe impl Send for ImgCopyWork {}

impl ImgCopyWork {
    pub unsafe fn new() -> Self {
        Self {
            src: ptr::null_mut(),
            dst: ptr::null_mut(),
            width: 0,
            stride: 0,
            bpp: 0,
            rects: vec![],
            _priv: (),
        }
    }
}

impl CpuWork for ImgCopyWork {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>> {
        unsafe extern "C" {
            fn memcpy(dst: *mut u8, src: *const u8, n: usize);
        }
        zone!("ImgCopyWork");
        let start = Instant::now();
        let mut total = 0;
        let mut calls = 0;
        for rect in &self.rects {
            let mut offset = rect.y1() * self.stride + rect.x1() * self.bpp;
            if rect.width() == self.width {
                let offset = offset as usize;
                let len = rect.height() * self.stride;
                unsafe {
                    memcpy(self.dst.add(offset), self.src.add(offset), len as _);
                }
                total += len;
                calls += 1;
            } else {
                let len = rect.width() * self.bpp;
                for _ in 0..rect.height() {
                    unsafe {
                        memcpy(
                            self.dst.add(offset as _),
                            self.src.add(offset as _),
                            len as _,
                        );
                    }
                    offset += self.stride;
                    total += len;
                    calls += 1;
                }
            }
        }
        log::info!(
            "ImgCopyWork took {:?} for {:?} with {} calls",
            start.elapsed(),
            total,
            calls
        );
        None
    }
}

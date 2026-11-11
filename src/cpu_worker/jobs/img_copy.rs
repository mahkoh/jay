use crate::cpu_worker::AsyncCpuWork;
use crate::cpu_worker::CpuWork;
use crate::rect::Rect;
use std::ptr;

pub struct ImgCopyWork {
    pub src: *mut u8,
    pub dst: *mut u8,
    pub width: usize,
    pub stride: usize,
    pub bpp: usize,
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
        zone!("ImgCopyWork");
        for rect in &self.rects {
            let x1 = rect.x1() as usize;
            let y1 = rect.y1() as usize;
            let width = rect.width() as usize;
            let height = rect.height() as usize;
            let mut offset = y1 * self.stride + x1 * self.bpp;
            if width == self.width {
                let len = height * self.stride;
                unsafe {
                    ptr::copy_nonoverlapping(self.src.add(offset), self.dst.add(offset), len);
                }
            } else {
                let len = width * self.bpp;
                for _ in 0..height {
                    unsafe {
                        ptr::copy_nonoverlapping(self.src.add(offset), self.dst.add(offset), len);
                    }
                    offset += self.stride;
                }
            }
        }
        None
    }
}

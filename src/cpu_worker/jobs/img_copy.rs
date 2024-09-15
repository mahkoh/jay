use {
    crate::{
        cpu_worker::{AsyncCpuWork, CpuWork},
        rect::Rect,
    },
    std::ptr,
};

#[expect(clippy::manual_non_exhaustive)]
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
        zone!("ImgCopyWork");
        for rect in &self.rects {
            let mut offset = rect.y1() * self.stride + rect.x1() * self.bpp;
            if rect.width() == self.width {
                let offset = offset as usize;
                let len = rect.height() * self.stride;
                unsafe {
                    ptr::copy_nonoverlapping(self.src.add(offset), self.dst.add(offset), len as _);
                }
            } else {
                let len = rect.width() * self.bpp;
                for _ in 0..rect.height() {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            self.src.add(offset as _),
                            self.dst.add(offset as _),
                            len as _,
                        );
                    }
                    offset += self.stride;
                }
            }
        }
        None
    }
}

use crate::render::egl::display::EglDisplay;
use crate::render::egl::sys::{EGLImageKHR, EGL_FALSE};
use crate::render::egl::PROCS;
use std::rc::Rc;

pub struct EglImage {
    pub dpy: Rc<EglDisplay>,
    pub img: EGLImageKHR,
    pub width: i32,
    pub height: i32,
}

impl Drop for EglImage {
    fn drop(&mut self) {
        unsafe {
            if PROCS.eglDestroyImageKHR(self.dpy.dpy, self.img) == EGL_FALSE {
                log::warn!("`eglDestroyImageKHR` failed");
            }
        }
    }
}

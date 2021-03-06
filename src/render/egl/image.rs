use {
    crate::render::egl::{
        display::EglDisplay,
        sys::{EGLImageKHR, EGL_FALSE},
        PROCS,
    },
    std::rc::Rc,
};

pub struct EglImage {
    pub dpy: Rc<EglDisplay>,
    pub img: EGLImageKHR,
    pub width: i32,
    pub height: i32,
    pub external_only: bool,
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

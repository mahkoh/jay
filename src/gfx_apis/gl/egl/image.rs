use {
    crate::{
        gfx_apis::gl::egl::{
            display::EglDisplay,
            sys::{EGLImageKHR, EGL_FALSE},
            PROCS,
        },
        video::dmabuf::DmaBuf,
    },
    std::rc::Rc,
};

pub struct EglImage {
    pub dpy: Rc<EglDisplay>,
    pub img: EGLImageKHR,
    pub external_only: bool,
    pub dmabuf: DmaBuf,
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

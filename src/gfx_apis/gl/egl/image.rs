use {
    crate::{
        gfx_apis::gl::egl::{
            display::EglDisplay,
            sys::{EGLImageKHR, EGL_FALSE},
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
            if self.dpy.procs.eglDestroyImageKHR(self.dpy.dpy, self.img) == EGL_FALSE {
                log::warn!("`eglDestroyImageKHR` failed");
            }
        }
    }
}

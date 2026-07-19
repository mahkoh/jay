use crate::gfx_apis::gl::egl::display::EglDisplay;
use crate::gfx_apis::gl::egl::sys::EGL_FALSE;
use crate::gfx_apis::gl::egl::sys::EGLImageKHR;
use crate::video::dmabuf::DmaBuf;
use std::rc::Rc;

pub struct EglImage {
    pub dpy: Rc<EglDisplay>,
    pub img: EGLImageKHR,
    pub external_only: bool,
    pub dmabuf: Rc<DmaBuf>,
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

use crate::render::egl::display::EglDisplay;
use crate::render::egl::sys::{
    eglDestroyContext, eglMakeCurrent, EGLContext, EGLSurface, EGL_FALSE, EGL_TRUE,
};
use crate::render::ext::GlExt;
use crate::render::RenderError;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct EglContext {
    pub dpy: Rc<EglDisplay>,
    pub ext: GlExt,
    pub ctx: EGLContext,
}

impl Drop for EglContext {
    fn drop(&mut self) {
        unsafe {
            if eglDestroyContext(self.dpy.dpy, self.ctx) != EGL_TRUE {
                log::warn!("`eglDestroyContext` failed");
            }
        }
    }
}

#[thread_local]
static mut CURRENT: EGLContext = EGLContext::none();

impl EglContext {
    #[inline]
    pub fn with_current<T, F: FnOnce() -> Result<T, RenderError>>(
        &self,
        f: F,
    ) -> Result<T, RenderError> {
        unsafe {
            if CURRENT == self.ctx {
                return f();
            }
            self.with_current_slow(f)
        }
    }

    #[cold]
    unsafe fn with_current_slow<T, F: FnOnce() -> Result<T, RenderError>>(
        &self,
        f: F,
    ) -> Result<T, RenderError> {
        if eglMakeCurrent(
            self.dpy.dpy,
            EGLSurface::none(),
            EGLSurface::none(),
            self.ctx,
        ) == EGL_FALSE
        {
            return Err(RenderError::MakeCurrent);
        }
        let prev = CURRENT;
        CURRENT = self.ctx;
        let res = f();
        if eglMakeCurrent(self.dpy.dpy, EGLSurface::none(), EGLSurface::none(), prev) == EGL_FALSE {
            panic!("Could not restore EGLContext");
        }
        CURRENT = prev;
        res
    }
}

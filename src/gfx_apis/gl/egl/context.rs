use {
    crate::{
        gfx_api::{GfxFormat, ResetStatus},
        gfx_apis::gl::{
            egl::{
                display::EglDisplay,
                sys::{
                    eglDestroyContext, eglMakeCurrent, EGLContext, EGLSurface, EGL_FALSE, EGL_TRUE,
                },
                PROCS,
            },
            ext::{GlExt, EXT_CREATE_CONTEXT_ROBUSTNESS},
            sys::{
                GL_GUILTY_CONTEXT_RESET_ARB, GL_INNOCENT_CONTEXT_RESET_ARB,
                GL_UNKNOWN_CONTEXT_RESET_ARB,
            },
            RenderError,
        },
    },
    ahash::AHashMap,
    std::rc::Rc,
};

#[derive(Debug, Clone)]
pub struct EglContext {
    pub dpy: Rc<EglDisplay>,
    pub ext: GlExt,
    pub ctx: EGLContext,
    pub formats: Rc<AHashMap<u32, GfxFormat>>,
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
    pub fn reset_status(&self) -> Option<ResetStatus> {
        if !self.dpy.exts.contains(EXT_CREATE_CONTEXT_ROBUSTNESS) {
            return None;
        }
        let status = self.with_current(|| unsafe {
            let status = match PROCS.glGetGraphicsResetStatusKHR() {
                0 => return Ok(None),
                GL_GUILTY_CONTEXT_RESET_ARB => ResetStatus::Guilty,
                GL_INNOCENT_CONTEXT_RESET_ARB => ResetStatus::Innocent,
                GL_UNKNOWN_CONTEXT_RESET_ARB => ResetStatus::Unknown,
                n => ResetStatus::Other(n),
            };
            Ok(Some(status))
        });
        status.unwrap_or_default()
    }

    #[inline]
    pub(in crate::gfx_apis::gl) fn with_current<T, F: FnOnce() -> Result<T, RenderError>>(
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

use {
    crate::{
        gfx_apis::gl::{
            egl::context::EglContext,
            sys::{
                EGLBoolean, EGLSyncKHR, EGL_NONE, EGL_SYNC_NATIVE_FENCE_ANDROID,
                EGL_SYNC_NATIVE_FENCE_FD_ANDROID, EGL_TRUE,
            },
            RenderError,
        },
        utils::errorfmt::ErrorFmt,
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct EglSync {
    ctx: Rc<EglContext>,
    sync: EGLSyncKHR,
}

impl EglContext {
    pub fn export_sync_file(self: &Rc<Self>) -> Result<OwnedFd, RenderError> {
        self.create_sync(None)?.export_sync_file()
    }

    pub fn create_sync(self: &Rc<Self>, file: Option<OwnedFd>) -> Result<EglSync, RenderError> {
        let mut attribs = [EGL_NONE; 3];
        if let Some(file) = &file {
            attribs[0] = EGL_SYNC_NATIVE_FENCE_FD_ANDROID;
            attribs[1] = file.raw();
        }
        self.with_current(|| unsafe {
            let sync = self.dpy.procs.eglCreateSyncKHR(
                self.dpy.dpy,
                EGL_SYNC_NATIVE_FENCE_ANDROID,
                attribs.as_ptr(),
            );
            if sync.is_null() {
                Err(RenderError::CreateEglSync)
            } else {
                if let Some(file) = file {
                    file.unwrap();
                }
                Ok(EglSync {
                    ctx: self.clone(),
                    sync,
                })
            }
        })
    }
}

impl EglSync {
    pub fn wait(&self) {
        let res = self.ctx.with_current(|| unsafe {
            let res = self
                .ctx
                .dpy
                .procs
                .eglWaitSyncKHR(self.ctx.dpy.dpy, self.sync, 0);
            if res as EGLBoolean == EGL_TRUE {
                Ok(())
            } else {
                Err(RenderError::WaitSync)
            }
        });
        if let Err(e) = res {
            log::warn!("Could not insert wait point: {}", ErrorFmt(e));
        }
    }

    pub fn export_sync_file(&self) -> Result<OwnedFd, RenderError> {
        self.ctx.with_current(|| unsafe {
            let fd = self
                .ctx
                .dpy
                .procs
                .eglDupNativeFenceFDANDROID(self.ctx.dpy.dpy, self.sync);
            if fd == -1 {
                Err(RenderError::ExportSyncFile)
            } else {
                Ok(OwnedFd::new(fd))
            }
        })
    }
}

impl Drop for EglSync {
    fn drop(&mut self) {
        let res = self.ctx.with_current(|| unsafe {
            let res = self
                .ctx
                .dpy
                .procs
                .eglDestroySyncKHR(self.ctx.dpy.dpy, self.sync);
            if res == EGL_TRUE {
                Ok(())
            } else {
                Err(RenderError::DestroyEglSync)
            }
        });
        if let Err(e) = res {
            log::error!("{}", ErrorFmt(e));
        }
    }
}

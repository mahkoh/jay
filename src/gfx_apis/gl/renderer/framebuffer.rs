use {
    crate::{
        format::Format,
        gfx_api::{
            AcquireSync, AsyncShmGfxTextureCallback, GfxApiOpt, GfxBlendBuffer, GfxError,
            GfxFramebuffer, GfxInternalFramebuffer, GfxStagingBuffer, PendingShmTransfer,
            ReleaseSync, ShmMemory, SyncFile,
        },
        gfx_apis::gl::{
            RenderError,
            gl::{
                frame_buffer::GlFrameBuffer,
                sys::{GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER},
            },
            handle_explicit_sync,
            renderer::context::GlRenderContext,
            run_ops,
            sys::{GL_ONE, GL_ONE_MINUS_SRC_ALPHA},
        },
        rect::Region,
        theme::Color,
    },
    std::{
        cell::Cell,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Framebuffer {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: GlFrameBuffer,
}

impl Debug for Framebuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Framebuffer").finish_non_exhaustive()
    }
}

impl Framebuffer {
    pub fn copy_to_shm(&self, shm: &[Cell<u8>]) -> Result<(), RenderError> {
        let format = self.gl.rb.format;
        let Some(shm_info) = &format.shm_info else {
            return Err(RenderError::UnsupportedShmFormat(format.name));
        };
        let gles = self.ctx.ctx.dpy.gles;
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                (gles.glBindFramebuffer)(GL_FRAMEBUFFER, self.gl.fbo);
                (gles.glViewport)(0, 0, self.gl.width, self.gl.height);
                (gles.glReadnPixels)(
                    0,
                    0,
                    self.gl.width,
                    self.gl.height,
                    shm_info.gl_format as _,
                    shm_info.gl_type as _,
                    shm.len() as _,
                    shm.as_ptr() as _,
                );
            }
            Ok(())
        });
        Ok(())
    }

    pub fn render(
        &self,
        acquire_sync: AcquireSync,
        ops: &[GfxApiOpt],
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, RenderError> {
        let gles = self.ctx.ctx.dpy.gles;
        self.ctx.ctx.with_current(|| {
            handle_explicit_sync(&self.ctx, self.gl.rb._img.as_ref(), &acquire_sync);
            unsafe {
                (gles.glBindFramebuffer)(GL_FRAMEBUFFER, self.gl.fbo);
                (gles.glViewport)(0, 0, self.gl.width, self.gl.height);
                if let Some(c) = clear {
                    (gles.glClearColor)(c.r, c.g, c.b, c.a);
                    (gles.glClear)(GL_COLOR_BUFFER_BIT);
                }
                (gles.glBlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            let fd = run_ops(self, ops);
            if fd.is_none() {
                unsafe {
                    (gles.glFinish)();
                }
            }
            Ok(fd)
        })
    }
}

impl GfxFramebuffer for Framebuffer {
    fn physical_size(&self) -> (i32, i32) {
        (self.gl.width, self.gl.height)
    }

    fn render_with_region(
        self: Rc<Self>,
        acquire_sync: AcquireSync,
        _release_sync: ReleaseSync,
        ops: &[GfxApiOpt],
        clear: Option<&Color>,
        _region: &Region,
        _blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
    ) -> Result<Option<SyncFile>, GfxError> {
        (*self)
            .render(acquire_sync, ops, clear)
            .map_err(|e| e.into())
    }

    fn format(&self) -> &'static Format {
        self.gl.rb.format
    }
}

impl GfxInternalFramebuffer for Framebuffer {
    fn into_fb(self: Rc<Self>) -> Rc<dyn GfxFramebuffer> {
        self
    }

    fn stride(&self) -> i32 {
        self.gl.rb.stride
    }

    fn staging_size(&self) -> usize {
        0
    }

    fn download(
        self: Rc<Self>,
        _staging: &Rc<dyn GfxStagingBuffer>,
        _callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        _damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError> {
        let mut res = Ok(());
        mem.access(&mut |mem| res = self.copy_to_shm(mem))
            .map_err(RenderError::AccessFailed)?;
        res?;
        Ok(None)
    }
}

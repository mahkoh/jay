use {
    crate::{
        format::Format,
        gfx_api::{GfxApiOpt, GfxError, GfxFramebuffer, SyncFile},
        gfx_apis::gl::{
            gl::{
                frame_buffer::GlFrameBuffer,
                sys::{GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER},
            },
            renderer::context::GlRenderContext,
            run_ops,
            sys::{GL_ONE, GL_ONE_MINUS_SRC_ALPHA},
            RenderError,
        },
        theme::Color,
    },
    std::{
        cell::Cell,
        fmt::{Debug, Formatter},
        mem,
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
    pub fn copy_to_shm(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: &Format,
        shm: &[Cell<u8>],
    ) -> Result<(), RenderError> {
        let Some(shm_info) = &format.shm_info else {
            return Err(RenderError::UnsupportedShmFormat(format.name));
        };
        let gles = self.ctx.ctx.dpy.gles;
        let y = self.gl.height - y - height;
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                (gles.glBindFramebuffer)(GL_FRAMEBUFFER, self.gl.fbo);
                (gles.glViewport)(0, 0, self.gl.width, self.gl.height);
                (gles.glReadnPixels)(
                    x,
                    y,
                    width,
                    height,
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
        mut ops: Vec<GfxApiOpt>,
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, RenderError> {
        let gles = self.ctx.ctx.dpy.gles;
        let res = self.ctx.ctx.with_current(|| {
            unsafe {
                (gles.glBindFramebuffer)(GL_FRAMEBUFFER, self.gl.fbo);
                (gles.glViewport)(0, 0, self.gl.width, self.gl.height);
                if let Some(c) = clear {
                    (gles.glClearColor)(c.r, c.g, c.b, c.a);
                    (gles.glClear)(GL_COLOR_BUFFER_BIT);
                }
                (gles.glBlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            let fd = run_ops(self, &ops);
            if fd.is_none() {
                unsafe {
                    (gles.glFlush)();
                }
            }
            Ok(fd)
        });
        ops.clear();
        *self.ctx.gfx_ops.borrow_mut() = ops;
        res
    }
}

impl GfxFramebuffer for Framebuffer {
    fn take_render_ops(&self) -> Vec<GfxApiOpt> {
        mem::take(&mut *self.ctx.gfx_ops.borrow_mut())
    }

    fn physical_size(&self) -> (i32, i32) {
        (self.gl.width, self.gl.height)
    }

    fn render(
        &self,
        ops: Vec<GfxApiOpt>,
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.render(ops, clear).map_err(|e| e.into())
    }

    fn copy_to_shm(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        _stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        (*self)
            .copy_to_shm(x, y, width, height, format, shm)
            .map_err(|e| e.into())
    }

    fn format(&self) -> &'static Format {
        self.gl.rb.format
    }
}

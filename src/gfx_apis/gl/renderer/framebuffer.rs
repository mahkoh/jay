use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_description::LinearColorDescription;
use crate::cmm::cmm_eotf::Eotf;
use crate::format::Format;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::AsyncShmGfxTextureCallback;
use crate::gfx_api::FdSync;
use crate::gfx_api::GfxApiOp;
use crate::gfx_api::GfxBlendBuffer;
use crate::gfx_api::GfxError;
use crate::gfx_api::GfxFramebuffer;
use crate::gfx_api::GfxInternalFramebuffer;
use crate::gfx_api::GfxStagingBuffer;
use crate::gfx_api::PendingShmTransfer;
use crate::gfx_api::ReleaseSync;
use crate::gfx_api::ShmMemory;
use crate::gfx_apis::gl::RenderError;
use crate::gfx_apis::gl::gl::frame_buffer::GlFrameBuffer;
use crate::gfx_apis::gl::gl::sys::GL_COLOR_BUFFER_BIT;
use crate::gfx_apis::gl::gl::sys::GL_FRAMEBUFFER;
use crate::gfx_apis::gl::handle_explicit_sync;
use crate::gfx_apis::gl::renderer::context::GlRenderContext;
use crate::gfx_apis::gl::run_ops;
use crate::gfx_apis::gl::sys::GL_ONE;
use crate::gfx_apis::gl::sys::GL_ONE_MINUS_SRC_ALPHA;
use crate::rect::Region;
use crate::theme::Color;
use std::cell::Cell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::rc::Rc;

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
        ops: &[GfxApiOp],
        clear: Option<&Color>,
    ) -> Result<Option<FdSync>, RenderError> {
        let gles = self.ctx.ctx.dpy.gles;
        self.ctx.ctx.with_current(|| {
            handle_explicit_sync(&self.ctx, self.gl.rb._img.as_ref(), &acquire_sync);
            unsafe {
                (gles.glBindFramebuffer)(GL_FRAMEBUFFER, self.gl.fbo);
                (gles.glViewport)(0, 0, self.gl.width, self.gl.height);
                if let Some(c) = clear {
                    let [r, g, b, a] = c.to_array(Eotf::Gamma22);
                    (gles.glClearColor)(r, g, b, a);
                    (gles.glClear)(GL_COLOR_BUFFER_BIT);
                }
                (gles.glBlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            let sync = run_ops(self, ops);
            if sync.is_none() {
                unsafe {
                    (gles.glFinish)();
                }
            }
            Ok(sync)
        })
    }
}

impl GfxFramebuffer for Framebuffer {
    fn physical_size(&self) -> (i32, i32) {
        (self.gl.width, self.gl.height)
    }

    fn render_with_region_impl(
        self: Rc<Self>,
        acquire_sync: AcquireSync,
        _release_sync: ReleaseSync,
        _cd: &Rc<ColorDescription>,
        ops: &[GfxApiOp],
        clear: Option<&Color>,
        _clear_cd: &Rc<LinearColorDescription>,
        _region: &Region,
        _blend_buffer: Option<&Rc<dyn GfxBlendBuffer>>,
        _blend_cd: &Rc<ColorDescription>,
        _sync: &[FdSync],
    ) -> Result<Option<FdSync>, GfxError> {
        (*self)
            .render(acquire_sync, ops, clear)
            .map_err(|e| e.into())
    }

    fn format(&self) -> &'static Format {
        self.gl.rb.format
    }
}

impl GfxInternalFramebuffer for Framebuffer {
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

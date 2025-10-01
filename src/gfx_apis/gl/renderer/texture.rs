use {
    crate::{
        format::Format,
        gfx_api::{
            AsyncShmGfxTexture, AsyncShmGfxTextureCallback, GfxError, GfxStagingBuffer, GfxTexture,
            PendingShmTransfer, ShmGfxTexture, ShmMemory,
        },
        gfx_apis::gl::{
            RenderError,
            gl::texture::GlTexture,
            renderer::context::GlRenderContext,
            sys::{
                GL_CLAMP_TO_EDGE, GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_TEXTURE_WRAP_T,
                GL_UNPACK_ROW_LENGTH_EXT, GLint,
            },
        },
        rect::Region,
        video::dmabuf::DmaBuf,
    },
    std::{
        cell::Cell,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Texture {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: GlTexture,
    pub(in crate::gfx_apis::gl) format: &'static Format,
}

impl Debug for Texture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture").finish_non_exhaustive()
    }
}

impl Texture {
    pub fn width(&self) -> i32 {
        self.gl.width
    }

    pub fn height(&self) -> i32 {
        self.gl.height
    }
}

impl GfxTexture for Texture {
    fn size(&self) -> (i32, i32) {
        (self.width(), self.height())
    }

    fn dmabuf(&self) -> Option<&DmaBuf> {
        self.gl.img.as_ref().map(|i| &i.dmabuf)
    }

    fn format(&self) -> &'static Format {
        self.format
    }
}

impl ShmGfxTexture for Texture {}

impl AsyncShmGfxTexture for Texture {
    fn async_upload(
        self: Rc<Self>,
        _staging: &Rc<dyn GfxStagingBuffer>,
        _callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        _damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError> {
        let mut res = Ok(());
        mem.access(&mut |data| {
            res = self.clone().sync_upload(data, Region::default());
        })
        .map_err(RenderError::AccessFailed)?;
        res.map(|_| None)
    }

    fn sync_upload(self: Rc<Self>, data: &[Cell<u8>], _damage: Region) -> Result<(), GfxError> {
        let shm_info = self.format.shm_info.as_ref().unwrap();
        if (self.gl.stride * self.gl.height) as usize > data.len() {
            return Err(RenderError::SmallImageBuffer.into());
        }
        let gles = self.ctx.ctx.dpy.gles;
        self.ctx.ctx.with_current(|| unsafe {
            (gles.glBindTexture)(GL_TEXTURE_2D, self.gl.tex);
            (gles.glTexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gles.glTexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gles.glPixelStorei)(
                GL_UNPACK_ROW_LENGTH_EXT,
                self.gl.stride / self.format.bpp as GLint,
            );
            (gles.glTexImage2D)(
                GL_TEXTURE_2D,
                0,
                shm_info.gl_format,
                self.gl.width,
                self.gl.height,
                0,
                shm_info.gl_format as _,
                shm_info.gl_type as _,
                data.as_ptr() as _,
            );
            (gles.glPixelStorei)(GL_UNPACK_ROW_LENGTH_EXT, 0);
            (gles.glBindTexture)(GL_TEXTURE_2D, 0);
            Ok(())
        })?;
        self.gl.contents_valid.set(true);
        Ok(())
    }

    fn compatible_with(
        &self,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> bool {
        format == self.gl.format
            && width == self.gl.width
            && height == self.gl.height
            && stride == self.gl.stride
    }
}

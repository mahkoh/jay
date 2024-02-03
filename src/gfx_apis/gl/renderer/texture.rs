use {
    crate::{
        gfx_api::GfxTexture,
        gfx_apis::gl::{gl::texture::GlTexture, renderer::context::GlRenderContext},
    },
    std::{
        any::Any,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Texture {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: GlTexture,
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

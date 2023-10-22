use {
    crate::gfx_apis::gl::{gl::texture::GlTexture, renderer::context::RenderContext},
    std::{
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Texture {
    pub(crate) ctx: Rc<RenderContext>,
    pub(crate) gl: GlTexture,
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

use {
    crate::render::{gl::texture::GlTexture, renderer::context::RenderContext},
    std::{
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Texture {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: GlTexture,
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

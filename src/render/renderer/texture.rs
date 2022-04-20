use {
    crate::render::{gl::texture::GlTexture, renderer::context::RenderContext},
    std::rc::Rc,
};

pub struct Texture {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: GlTexture,
}

impl Texture {
    pub fn width(&self) -> i32 {
        self.gl.width
    }

    pub fn height(&self) -> i32 {
        self.gl.height
    }
}

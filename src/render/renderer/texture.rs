use crate::render::gl::texture::GlTexture;
use crate::render::renderer::context::RenderContext;
use std::rc::Rc;

pub struct Texture {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: GlTexture,
}

use {
    crate::render::{
        egl::image::EglImage, gl::texture::GlTexture, RenderContext, RenderError, Texture,
    },
    std::rc::Rc,
};

pub struct Image {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: Rc<EglImage>,
}

impl Image {
    pub fn width(&self) -> i32 {
        self.gl.width
    }

    pub fn height(&self) -> i32 {
        self.gl.height
    }

    pub fn to_texture(self: &Rc<Self>) -> Result<Rc<Texture>, RenderError> {
        Ok(Rc::new(Texture {
            ctx: self.ctx.clone(),
            gl: GlTexture::import_img(&self.ctx.ctx, &self.gl)?,
        }))
    }
}

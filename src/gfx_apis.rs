use {
    crate::{
        gfx_api::{GfxContext, GfxError},
        video::drm::Drm,
    },
    std::rc::Rc,
};

pub mod gl;

pub fn create_gfx_context(drm: &Drm) -> Result<Rc<dyn GfxContext>, GfxError> {
    gl::create_gfx_context(drm)
}

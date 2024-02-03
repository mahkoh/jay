use {
    crate::{
        gfx_api::{GfxContext, GfxError},
        video::drm::{wait_for_syncobj::WaitForSyncObj, Drm},
    },
    std::rc::Rc,
};

pub mod gl;
mod vulkan;

pub fn create_gfx_context(
    drm: &Drm,
    wait_for_sync_obj: &Rc<WaitForSyncObj>,
) -> Result<Rc<dyn GfxContext>, GfxError> {
    if false {
        gl::create_gfx_context(drm)
    } else {
        vulkan::create_graphics_context(drm, wait_for_sync_obj)
    }
}

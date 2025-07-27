pub use vulkan::create_vulkan_allocator;
use {
    crate::{
        async_engine::AsyncEngine,
        gfx_api::{GfxContext, GfxError},
        io_uring::IoUring,
        pr_caps::PrCapsThread,
        utils::errorfmt::ErrorFmt,
        video::drm::Drm,
    },
    jay_config::video::GfxApi,
    std::rc::Rc,
};

pub mod gl;
mod vulkan;

pub fn create_gfx_context(
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
    drm: &Drm,
    api: GfxApi,
    caps_thread: Option<&PrCapsThread>,
) -> Result<Rc<dyn GfxContext>, GfxError> {
    let mut apis = [GfxApi::OpenGl, GfxApi::Vulkan];
    apis.sort_by_key(|&a| if a == api { -1 } else { a as i32 });
    let mut last_err = None;
    for software in [false, true] {
        for api in apis {
            let res = create_gfx_context_(eng, ring, drm, api, caps_thread, software);
            match res {
                Ok(_) => {
                    log::info!("Created a {api:?} renderer");
                    if software {
                        log::warn!("Renderer uses software rendering");
                    }
                    return res;
                }
                Err(e) => {
                    log::warn!("Could not create {:?} API: {}", api, ErrorFmt(&e));
                    last_err = Some(e);
                }
            }
        }
    }
    Err(last_err.unwrap())
}

fn create_gfx_context_(
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
    drm: &Drm,
    api: GfxApi,
    caps_thread: Option<&PrCapsThread>,
    software: bool,
) -> Result<Rc<dyn GfxContext>, GfxError> {
    match api {
        GfxApi::OpenGl => gl::create_gfx_context(drm, software),
        GfxApi::Vulkan => vulkan::create_graphics_context(eng, ring, drm, caps_thread, software),
        _ => unreachable!(),
    }
}

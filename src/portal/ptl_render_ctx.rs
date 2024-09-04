use {
    crate::gfx_api::{GfxContext, GfxFormat},
    ahash::AHashMap,
    std::rc::Rc,
    uapi::c,
};

pub struct PortalRenderCtx {
    pub _dev_id: c::dev_t,
    pub ctx: Rc<dyn GfxContext>,
}

pub struct PortalServerRenderCtx {
    pub ctx: Rc<PortalRenderCtx>,
    pub usable_formats: Rc<AHashMap<u32, GfxFormat>>,
    pub server_formats: Option<AHashMap<u32, GfxFormat>>,
}

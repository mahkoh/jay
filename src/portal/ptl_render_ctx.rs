use {
    crate::{
        gfx_api::{GfxContext, GfxFormat},
        utils::bhash::BHashMap,
    },
    std::rc::Rc,
    uapi::c,
};

pub struct PortalRenderCtx {
    pub _dev_id: c::dev_t,
    pub ctx: Rc<dyn GfxContext>,
}

pub struct PortalServerRenderCtx {
    pub ctx: Rc<PortalRenderCtx>,
    pub usable_formats: Rc<BHashMap<u32, GfxFormat>>,
    pub server_formats: Option<BHashMap<u32, GfxFormat>>,
}

use crate::gfx_api::GfxContext;
use crate::gfx_api::GfxFormat;
use crate::utils::bhash::BHashMap;
use std::rc::Rc;
use uapi::c;

pub struct PortalRenderCtx {
    pub _dev_id: c::dev_t,
    pub ctx: Rc<dyn GfxContext>,
}

pub struct PortalServerRenderCtx {
    pub ctx: Rc<PortalRenderCtx>,
    pub usable_formats: Rc<BHashMap<u32, GfxFormat>>,
    pub server_formats: Option<BHashMap<u32, GfxFormat>>,
}

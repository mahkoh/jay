use {crate::gfx_apis::gl::RenderContext, std::rc::Rc, uapi::c};

pub struct PortalRenderCtx {
    pub dev_id: c::dev_t,
    pub ctx: Rc<RenderContext>,
}

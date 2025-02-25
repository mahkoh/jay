use {
    crate::{
        allocator::{AllocatorError, BO_USE_RENDERING, BufferObject, BufferUsage},
        format::XRGB8888,
        gfx_api::{AcquireSync, GfxError, ReleaseSync, needs_render_usage},
        scale::Scale,
        state::State,
        video::drm::DrmError,
    },
    indexmap::IndexMap,
    jay_config::video::Transform,
    std::{ops::Deref, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

#[derive(Debug, Error)]
pub enum ScreenshooterError {
    #[error("There is no render context")]
    NoRenderContext,
    #[error("Display is empty")]
    EmptyDisplay,
    #[error(transparent)]
    AllocatorError(#[from] AllocatorError),
    #[error(transparent)]
    RenderError(#[from] GfxError),
    #[error(transparent)]
    DrmError(#[from] DrmError),
    #[error("Render context does not support XRGB8888")]
    XRGB8888,
    #[error("Render context supports no modifiers for XRGB8888 rendering")]
    Modifiers,
}

pub struct Screenshot {
    pub drm: Option<Rc<OwnedFd>>,
    pub bo: Rc<dyn BufferObject>,
}

pub fn take_screenshot(
    state: &State,
    include_cursor: bool,
) -> Result<Screenshot, ScreenshooterError> {
    let ctx = match state.render_ctx.get() {
        Some(ctx) => ctx,
        _ => return Err(ScreenshooterError::NoRenderContext),
    };
    let extents = state.root.extents.get();
    if extents.is_empty() {
        return Err(ScreenshooterError::EmptyDisplay);
    }
    let formats = ctx.formats();
    let modifiers: IndexMap<_, _> = match formats.get(&XRGB8888.drm) {
        None => return Err(ScreenshooterError::XRGB8888),
        Some(f) => f
            .write_modifiers
            .iter()
            .filter(|(m, _)| f.read_modifiers.contains(*m))
            .collect(),
    };
    if modifiers.is_empty() {
        return Err(ScreenshooterError::Modifiers);
    }
    let mut usage = BO_USE_RENDERING;
    if !needs_render_usage(modifiers.values().copied()) {
        usage = BufferUsage::none();
    }
    let modifiers: Vec<_> = modifiers.keys().copied().copied().collect();
    let allocator = ctx.allocator();
    let bo = allocator.create_bo(
        &state.dma_buf_ids,
        extents.width(),
        extents.height(),
        XRGB8888,
        &modifiers,
        usage,
    )?;
    let fb = ctx.clone().dmabuf_fb(bo.dmabuf())?;
    fb.render_node(
        AcquireSync::Unnecessary,
        ReleaseSync::Implicit,
        state.root.deref(),
        state,
        Some(state.root.extents.get()),
        Scale::from_int(1),
        include_cursor,
        true,
        false,
        false,
        Transform::None,
        None,
    )?;
    let drm = match allocator.drm() {
        Some(drm) => Some(drm.dup_render()?.fd().clone()),
        _ => None,
    };
    Ok(Screenshot { drm, bo })
}

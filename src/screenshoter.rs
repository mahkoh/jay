use {
    crate::{
        allocator::{AllocatorError, BufferObject, BO_USE_RENDERING},
        format::XRGB8888,
        gfx_api::GfxError,
        scale::Scale,
        state::State,
        video::drm::DrmError,
    },
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
    let modifiers: Vec<_> = match formats.get(&XRGB8888.drm) {
        None => return Err(ScreenshooterError::XRGB8888),
        Some(f) => f
            .write_modifiers
            .intersection(&f.read_modifiers)
            .copied()
            .collect(),
    };
    if modifiers.is_empty() {
        return Err(ScreenshooterError::Modifiers);
    }
    let allocator = ctx.allocator();
    let bo = allocator.create_bo(
        &state.dma_buf_ids,
        extents.width(),
        extents.height(),
        XRGB8888,
        &modifiers,
        BO_USE_RENDERING,
    )?;
    let fb = ctx.clone().dmabuf_fb(bo.dmabuf())?;
    fb.render_node(
        state.root.deref(),
        state,
        Some(state.root.extents.get()),
        None,
        Scale::from_int(1),
        include_cursor,
        true,
        false,
        Transform::None,
    )?;
    let drm = match allocator.drm() {
        Some(drm) => Some(drm.dup_render()?.fd().clone()),
        _ => None,
    };
    Ok(Screenshot { drm, bo })
}

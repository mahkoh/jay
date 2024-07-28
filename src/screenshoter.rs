use {
    crate::{
        format::XRGB8888,
        gfx_api::GfxError,
        scale::Scale,
        state::State,
        video::{
            drm::DrmError,
            gbm::{GbmBo, GbmError, GBM_BO_USE_RENDERING},
        },
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
    GbmError(#[from] GbmError),
    #[error(transparent)]
    RenderError(#[from] GfxError),
    #[error(transparent)]
    DrmError(#[from] DrmError),
    #[error("Render context does not support XRGB8888")]
    XRGB8888,
    #[error("Render context cannot render to XRGB8888")]
    NoModifiers,
}

pub struct Screenshot {
    pub drm: Rc<OwnedFd>,
    pub bo: GbmBo,
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
    let modifiers = match formats.get(&XRGB8888.drm) {
        None => return Err(ScreenshooterError::XRGB8888),
        Some(f) => &f.write_modifiers,
    };
    if modifiers.is_empty() {
        return Err(ScreenshooterError::NoModifiers);
    }
    let gbm = ctx.gbm();
    let bo = gbm.create_bo(
        &state.dma_buf_ids,
        extents.width(),
        extents.height(),
        XRGB8888,
        modifiers,
        GBM_BO_USE_RENDERING,
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
    let drm = gbm.drm.dup_render()?.fd().clone();
    Ok(Screenshot { drm, bo })
}

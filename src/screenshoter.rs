use {
    crate::{
        format::XRGB8888,
        gfx_api::GfxError,
        scale::Scale,
        state::State,
        video::{
            drm::DrmError,
            gbm::{GbmBo, GbmError, GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING},
        },
    },
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
}

pub struct Screenshot {
    pub drm: Rc<OwnedFd>,
    pub bo: GbmBo,
}

pub fn take_screenshot(state: &State) -> Result<Screenshot, ScreenshooterError> {
    let ctx = match state.render_ctx.get() {
        Some(ctx) => ctx,
        _ => return Err(ScreenshooterError::NoRenderContext),
    };
    let extents = state.root.extents.get();
    if extents.is_empty() {
        return Err(ScreenshooterError::EmptyDisplay);
    }
    let gbm = ctx.gbm();
    let bo = gbm.create_bo(
        extents.width(),
        extents.height(),
        XRGB8888,
        &[],
        GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR,
    )?;
    let fb = ctx.clone().dmabuf_fb(bo.dmabuf())?;
    fb.render_node(
        state.root.deref(),
        state,
        Some(state.root.extents.get()),
        None,
        Scale::from_int(1),
        true,
    );
    let drm = gbm.drm.dup_render()?.fd().clone();
    Ok(Screenshot { drm, bo })
}

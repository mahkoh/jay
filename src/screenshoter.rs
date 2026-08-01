use crate::allocator::AllocatorError;
use crate::allocator::BO_USE_RENDERING;
use crate::allocator::BufferObject;
use crate::allocator::BufferUsage;
use crate::cmm::cmm_eotf::Eotf;
use crate::format::Format;
use crate::format::XRGB8888;
use crate::format::XRGB2101010;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::GfxError;
use crate::gfx_api::ReleaseSync;
use crate::gfx_api::ScalingFilter;
use crate::gfx_api::needs_render_usage;
use crate::scale::Scale;
use crate::state::State;
use crate::tree::Transform;
use crate::tree::TreeTimeline::RenderTL;
use crate::video::drm::DrmError;
use indexmap::IndexMap;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

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
    #[error("Render context does not support {}", .0.name)]
    UnsupportedFormat(&'static Format),
    #[error("Render context supports no modifiers for {} rendering", .0.name)]
    Modifiers(&'static Format),
    #[error("Renderer does not support color management")]
    NoColorManagementSupport,
}

pub struct Screenshot {
    pub drm: Option<Rc<OwnedFd>>,
    pub bo: Rc<dyn BufferObject>,
}

pub fn take_screenshot(
    state: &State,
    include_cursor: bool,
    hdr10: bool,
) -> Result<Screenshot, ScreenshooterError> {
    let ctx = match state.render_ctx.get() {
        Some(ctx) => ctx,
        _ => return Err(ScreenshooterError::NoRenderContext),
    };
    if hdr10 && !ctx.supports_color_management() {
        return Err(ScreenshooterError::NoColorManagementSupport);
    }
    let extents = state.root.node_state[RenderTL].extents.get();
    if extents.is_empty() {
        return Err(ScreenshooterError::EmptyDisplay);
    }
    // HDR10 output is 10-bit-per-channel BT.2020 primaries with an ST 2084 (PQ)
    // transfer function. `windows_bt2100` is exactly that color description, so
    // we reuse it here instead of introducing a separate one.
    let format = if hdr10 { XRGB2101010 } else { XRGB8888 };
    let (cd, blend_cd) = if hdr10 {
        let cd = state.color_manager.windows_bt2100();
        let blend_cd = state.color_manager.get_with_tf(cd, Eotf::Linear);
        (cd, blend_cd)
    } else {
        (
            state.color_manager.srgb_gamma22(),
            state.color_manager.srgb_linear().clone(),
        )
    };
    let formats = ctx.formats();
    let modifiers: IndexMap<_, _> = match formats.get(&format.drm) {
        None => return Err(ScreenshooterError::UnsupportedFormat(format)),
        Some(f) => f
            .write_modifiers
            .iter()
            .filter(|(m, _)| f.read_modifiers.contains(*m))
            .collect(),
    };
    if modifiers.is_empty() {
        return Err(ScreenshooterError::Modifiers(format));
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
        format,
        &modifiers,
        usage,
    )?;
    let fb = ctx.clone().dmabuf_fb(bo.dmabuf())?;
    fb.render_node(
        AcquireSync::Unnecessary,
        ReleaseSync::Implicit,
        cd,
        state.root.deref(),
        state,
        Some(state.root.node_state[RenderTL].extents.get()),
        Scale::from_int(1),
        ScalingFilter::Linear,
        include_cursor,
        true,
        false,
        false,
        Transform::None,
        None,
        &blend_cd,
        false,
    )?;
    let drm = match allocator.drm() {
        Some(drm) => Some(drm.dup_render()?.fd().clone()),
        _ => None,
    };
    Ok(Screenshot { drm, bo })
}

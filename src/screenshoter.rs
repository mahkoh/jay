use crate::allocator::Allocator;
use crate::allocator::AllocatorError;
use crate::allocator::BufferUsage;
use crate::cmm::cmm_eotf::Eotf;
use crate::format::XRGB8888;
use crate::format::XRGB2101010;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::AsyncShmGfxTextureCallback;
use crate::gfx_api::GfxError;
use crate::gfx_api::GfxFramebuffer;
use crate::gfx_api::PendingShmTransfer;
use crate::gfx_api::ReleaseSync;
use crate::gfx_api::STAGING_DOWNLOAD;
use crate::gfx_api::ScalingFilter;
use crate::gfx_api::ShmMemory;
use crate::gfx_api::ShmMemoryBacking;
use crate::scale::Scale;
use crate::state::State;
use crate::tree::Transform;
use crate::tree::TreeTimeline::RenderTL;
use crate::video::LINEAR_MODIFIER;
use crate::video::dmabuf::DmaBuf;
use jay_algorithms::mmap::Mmapped;
use jay_algorithms::mmap::mmap;
use jay_algorithms::oserror::OsError;
use std::cell::Cell;
use std::error::Error;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use uapi::c;

#[derive(Debug, Error)]
pub enum ScreenshooterError {
    #[error("There is no render context")]
    NoRenderContext,
    #[error("Display is empty")]
    EmptyDisplay,
    #[error("udmabuf is not available")]
    NoUdmabuf,
    #[error(transparent)]
    AllocatorError(#[from] AllocatorError),
    #[error(transparent)]
    RenderError(#[from] GfxError),
    #[error("Renderer does not support color management")]
    NoColorManagementSupport,
    #[error("Could not map the udmabuf")]
    Map(#[source] OsError),
}

pub trait ScreenshotCallback {
    fn completed(self: Rc<Self>, res: Result<Rc<DmaBuf>, ScreenshooterError>);
}

struct Download {
    buf: Rc<DmaBuf>,
    callback: Cell<Option<Rc<dyn ScreenshotCallback>>>,
}

impl AsyncShmGfxTextureCallback for Download {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>) {
        let Some(cb) = self.callback.take() else {
            return;
        };
        let res = res
            .map(|_| self.buf.clone())
            .map_err(ScreenshooterError::from);
        cb.completed(res);
    }
}

struct Memory {
    map: Mmapped,
}

impl Memory {
    fn data(&self) -> *const [Cell<u8>] {
        self.map.ptr as *const [Cell<u8>]
    }
}

impl ShmMemory for Memory {
    fn len(&self) -> usize {
        self.data().len()
    }

    fn safe_access(&self) -> ShmMemoryBacking {
        ShmMemoryBacking::Ptr(self.data())
    }

    fn access(&self, f: &mut dyn FnMut(&[Cell<u8>])) -> Result<(), Box<dyn Error + Sync + Send>> {
        f(unsafe { &*self.data() });
        Ok(())
    }
}

pub fn take_screenshot(
    state: &State,
    include_cursor: bool,
    hdr10: bool,
    callback: Rc<dyn ScreenshotCallback>,
) -> Result<Option<PendingShmTransfer>, ScreenshooterError> {
    let Some(ctx) = state.render_ctx.get() else {
        return Err(ScreenshooterError::NoRenderContext);
    };
    if hdr10 && !ctx.supports_color_management() {
        return Err(ScreenshooterError::NoColorManagementSupport);
    }
    let extents = state.root.node_state[RenderTL].extents.get();
    if extents.is_empty() {
        return Err(ScreenshooterError::EmptyDisplay);
    }
    let Some(udmabuf) = state.udmabuf.get() else {
        return Err(ScreenshooterError::NoUdmabuf);
    };
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
    let bo = udmabuf.create_bo(
        &state.dma_buf_ids,
        extents.width(),
        extents.height(),
        format,
        &[LINEAR_MODIFIER],
        BufferUsage::none(),
    )?;
    let buf = bo.dmabuf().clone();
    let plane = &buf.planes[0];
    let map = mmap(
        buf.height as usize * plane.stride as usize,
        c::PROT_READ | c::PROT_WRITE,
        c::MAP_SHARED,
        plane.fd.raw(),
        0,
    )
    .map_err(ScreenshooterError::Map)?;
    let mem = Rc::new(Memory { map });
    let fb = ctx.clone().create_internal_fb(
        &state.cpu_worker,
        extents.width(),
        extents.height(),
        plane.stride as i32,
        format,
    )?;
    (fb.clone() as Rc<dyn GfxFramebuffer>).render_node(
        AcquireSync::Unnecessary,
        ReleaseSync::None,
        cd,
        state.root.deref(),
        state,
        Some(extents),
        Scale::from_int(1),
        ScalingFilter::Linear,
        include_cursor,
        true,
        false,
        false,
        None,
        Transform::None,
        None,
        &blend_cd,
        false,
    )?;
    let download = Rc::new(Download {
        buf,
        callback: Cell::new(Some(callback)),
    });
    let staging = ctx.create_staging_buffer(fb.staging_size(), STAGING_DOWNLOAD);
    let region = fb.full_region();
    let pending = fb.download(&staging, download.clone(), mem, region)?;
    if pending.is_none() {
        let rtl = &state.run_toplevel;
        rtl.schedule(move || download.completed(Ok(())));
    }
    Ok(pending)
}

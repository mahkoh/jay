use {
    crate::{
        async_engine::{Phase, SpawnedFuture},
        backend::{
            BackendDrmDevice, BackendEvent, Connector, ConnectorEvent, ConnectorId,
            ConnectorKernelId, DrmDeviceId, HardwareCursor, MonitorInfo,
        },
        backends::metal::{MetalBackend, MetalError},
        drm_feedback::DrmFeedback,
        edid::Descriptor,
        format::{Format, ARGB8888, XRGB8888},
        gfx_api::{BufferPoints, GfxApiOpt, GfxContext, GfxFramebuffer, GfxRenderPass, GfxTexture},
        ifs::wp_presentation_feedback::{KIND_HW_COMPLETION, KIND_VSYNC},
        renderer::RenderResult,
        state::State,
        tree::OutputNode,
        udev::UdevDevice,
        utils::{
            asyncevent::AsyncEvent, bitflags::BitflagsExt, clonecell::CloneCell,
            copyhashmap::CopyHashMap, debug_fn::debug_fn, errorfmt::ErrorFmt, numcell::NumCell,
            opaque_cell::OpaqueCell, oserror::OsError, syncqueue::SyncQueue,
        },
        video::{
            dmabuf::DmaBufId,
            drm::{
                drm_mode_modeinfo, Change, ConnectorStatus, ConnectorType, DrmBlob, DrmConnector,
                DrmCrtc, DrmEncoder, DrmError, DrmEvent, DrmFramebuffer, DrmMaster, DrmModeInfo,
                DrmObject, DrmPlane, DrmProperty, DrmPropertyDefinition, DrmPropertyType,
                DrmVersion, PropBlob, DRM_CLIENT_CAP_ATOMIC, DRM_MODE_ATOMIC_ALLOW_MODESET,
                DRM_MODE_ATOMIC_NONBLOCK, DRM_MODE_PAGE_FLIP_EVENT,
            },
            gbm::{GbmDevice, GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING, GBM_BO_USE_SCANOUT},
            Modifier, INVALID_MODIFIER,
        },
    },
    ahash::{AHashMap, AHashSet},
    bstr::{BString, ByteSlice},
    indexmap::{indexset, IndexSet},
    jay_config::video::GfxApi,
    std::{
        cell::{Cell, RefCell},
        ffi::CString,
        fmt::{Debug, Formatter},
        mem,
        ops::DerefMut,
        rc::{Rc, Weak},
    },
    uapi::c::{self, dev_t},
};

pub struct PendingDrmDevice {
    pub id: DrmDeviceId,
    pub devnum: c::dev_t,
    pub devnode: CString,
}

#[derive(Debug)]
pub struct MetalRenderContext {
    pub dev_id: DrmDeviceId,
    pub gfx: Rc<dyn GfxContext>,
}

#[derive(Debug)]
pub struct MetalDrmDevice {
    pub backend: Rc<MetalBackend>,
    pub id: DrmDeviceId,
    pub devnum: c::dev_t,
    pub devnode: CString,
    pub master: Rc<DrmMaster>,
    pub crtcs: AHashMap<DrmCrtc, Rc<MetalCrtc>>,
    pub encoders: AHashMap<DrmEncoder, Rc<MetalEncoder>>,
    pub planes: AHashMap<DrmPlane, Rc<MetalPlane>>,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub cursor_width: u64,
    pub cursor_height: u64,
    pub gbm: GbmDevice,
    pub handle_events: HandleEvents,
    pub ctx: CloneCell<Rc<MetalRenderContext>>,
    pub on_change: OnChange<crate::backend::DrmEvent>,
    pub direct_scanout_enabled: Cell<Option<bool>>,
}

impl MetalDrmDevice {
    pub fn is_render_device(&self) -> bool {
        if let Some(ctx) = self.backend.ctx.get() {
            return ctx.dev_id == self.id;
        }
        false
    }
}

impl BackendDrmDevice for MetalDrmDevice {
    fn id(&self) -> DrmDeviceId {
        self.id
    }

    fn event(&self) -> Option<crate::backend::DrmEvent> {
        self.on_change.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.on_change.set(Some(cb));
    }

    fn dev_t(&self) -> dev_t {
        self.devnum
    }

    fn make_render_device(&self) {
        self.backend.make_render_device(&self, false);
    }

    fn set_gfx_api(&self, api: GfxApi) {
        self.backend.set_gfx_api(self, api)
    }

    fn gtx_api(&self) -> GfxApi {
        self.ctx.get().gfx.gfx_api()
    }

    fn version(&self) -> Result<DrmVersion, DrmError> {
        self.gbm.drm.version()
    }

    fn set_direct_scanout_enabled(&self, enabled: bool) {
        self.direct_scanout_enabled.set(Some(enabled));
    }
}

pub struct HandleEvents {
    pub handle_events: Cell<Option<SpawnedFuture<()>>>,
}

impl Debug for HandleEvents {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandleEvents").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct MetalDrmDeviceData {
    pub dev: Rc<MetalDrmDevice>,
    pub connectors: CopyHashMap<DrmConnector, Rc<MetalConnector>>,
    pub futures: CopyHashMap<DrmConnector, ConnectorFutures>,
    pub unprocessed_change: Cell<bool>,
}

#[derive(Debug)]
pub struct ConnectorDisplayData {
    pub crtc_id: MutableProperty<DrmCrtc>,
    pub crtcs: AHashMap<DrmCrtc, Rc<MetalCrtc>>,
    pub modes: Vec<DrmModeInfo>,
    pub mode: Option<Rc<DrmModeInfo>>,
    pub refresh: u32,

    pub monitor_manufacturer: String,
    pub monitor_name: String,
    pub monitor_serial_number: String,

    pub connection: ConnectorStatus,
    pub mm_width: u32,
    pub mm_height: u32,
    pub subpixel: u32,

    pub connector_type: ConnectorType,
    pub connector_type_id: u32,
}

impl ConnectorDisplayData {
    fn is_same_monitor(&self, other: &Self) -> bool {
        self.monitor_manufacturer == other.monitor_manufacturer
            && self.monitor_name == other.monitor_name
            && self.monitor_serial_number == other.monitor_serial_number
    }
}

#[derive(Debug)]
pub struct MetalConnector {
    pub id: DrmConnector,
    pub master: Rc<DrmMaster>,
    pub state: Rc<State>,

    pub dev: Rc<MetalDrmDevice>,
    pub backend: Rc<MetalBackend>,

    pub connector_id: ConnectorId,

    pub buffers: CloneCell<Option<Rc<[RenderBuffer; 2]>>>,
    pub next_buffer: NumCell<usize>,

    pub enabled: Cell<bool>,

    pub can_present: Cell<bool>,
    pub has_damage: Cell<bool>,
    pub cursor_changed: Cell<bool>,

    pub display: RefCell<ConnectorDisplayData>,

    pub connect_sent: Cell<bool>,

    pub primary_plane: CloneCell<Option<Rc<MetalPlane>>>,
    pub cursor_plane: CloneCell<Option<Rc<MetalPlane>>>,

    pub crtc: CloneCell<Option<Rc<MetalCrtc>>>,

    pub on_change: OnChange<ConnectorEvent>,

    pub present_trigger: AsyncEvent,

    pub render_result: RefCell<RenderResult>,

    pub cursor_generation: NumCell<u64>,
    pub cursor_x: Cell<i32>,
    pub cursor_y: Cell<i32>,
    pub cursor_enabled: Cell<bool>,
    pub cursor_buffers: CloneCell<Option<Rc<[RenderBuffer; 2]>>>,
    pub cursor_front_buffer: NumCell<usize>,
    pub cursor_swap_buffer: Cell<bool>,

    pub drm_feedback: CloneCell<Option<Rc<DrmFeedback>>>,
    pub scanout_buffers: RefCell<AHashMap<DmaBufId, DirectScanoutCache>>,
    pub active_framebuffer: OpaqueCell<Option<PresentFb>>,
    pub next_framebuffer: OpaqueCell<Option<PresentFb>>,
    pub direct_scanout_active: Cell<bool>,
}

#[derive(Debug)]
pub struct MetalHardwareCursor {
    pub generation: u64,
    pub connector: Rc<MetalConnector>,
    pub cursor_swap_buffer: Cell<bool>,
    pub cursor_enabled_pending: Cell<bool>,
    pub cursor_x_pending: Cell<i32>,
    pub cursor_y_pending: Cell<i32>,
    pub cursor_buffers: Rc<[RenderBuffer; 2]>,
    pub have_changes: Cell<bool>,
}

impl HardwareCursor for MetalHardwareCursor {
    fn set_enabled(&self, enabled: bool) {
        if self.cursor_enabled_pending.replace(enabled) != enabled {
            self.have_changes.set(true);
        }
    }

    fn get_buffer(&self) -> Rc<dyn GfxFramebuffer> {
        let buffer = (self.connector.cursor_front_buffer.get() + 1) % 2;
        self.cursor_buffers[buffer].render_fb()
    }

    fn set_position(&self, x: i32, y: i32) {
        self.cursor_x_pending.set(x);
        self.cursor_y_pending.set(y);
        self.have_changes.set(true);
    }

    fn swap_buffer(&self) {
        self.cursor_swap_buffer.set(true);
        self.have_changes.set(true);
    }

    fn commit(&self) {
        if self.generation != self.connector.cursor_generation.get() {
            return;
        }
        if !self.have_changes.take() {
            return;
        }
        self.connector
            .cursor_enabled
            .set(self.cursor_enabled_pending.get());
        self.connector.cursor_x.set(self.cursor_x_pending.get());
        self.connector.cursor_y.set(self.cursor_y_pending.get());
        if self.cursor_swap_buffer.take() {
            self.connector.cursor_swap_buffer.set(true);
        }
        self.connector.cursor_changed.set(true);
        if self.connector.can_present.get() {
            self.connector.schedule_present();
        }
    }

    fn max_size(&self) -> (i32, i32) {
        (
            self.connector.dev.cursor_width as _,
            self.connector.dev.cursor_height as _,
        )
    }
}

pub struct ConnectorFutures {
    pub present: SpawnedFuture<()>,
}

impl Debug for ConnectorFutures {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectorFutures").finish_non_exhaustive()
    }
}

pub struct OnChange<T> {
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
    pub events: SyncQueue<T>,
}

impl<T> OnChange<T> {
    pub fn send_event(&self, event: T) {
        self.events.push(event);
        if let Some(cb) = self.on_change.get() {
            cb();
        }
    }
}

impl<T> Default for OnChange<T> {
    fn default() -> Self {
        Self {
            on_change: Default::default(),
            events: Default::default(),
        }
    }
}

impl<T> Debug for OnChange<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.on_change.get() {
            None => f.write_str("None"),
            Some(_) => f.write_str("Some"),
        }
    }
}

#[derive(Debug)]
pub struct DirectScanoutCache {
    tex: Weak<dyn GfxTexture>,
    fb: Option<Rc<DrmFramebuffer>>,
}

#[derive(Debug)]
pub struct DirectScanoutData {
    tex: Rc<dyn GfxTexture>,
    fb: Rc<DrmFramebuffer>,
    dma_buf_id: DmaBufId,
    acquired: Cell<bool>,
}

impl Drop for DirectScanoutData {
    fn drop(&mut self) {
        if self.acquired.replace(false) {
            self.tex.reservations().release();
        }
    }
}

#[derive(Debug)]
pub struct PresentFb {
    fb: Rc<DrmFramebuffer>,
    direct_scanout_data: Option<DirectScanoutData>,
}

impl MetalConnector {
    async fn present_loop(self: Rc<Self>) {
        loop {
            self.present_trigger.triggered().await;
            let _ = self.present(true);
        }
    }

    fn send_hardware_cursor(self: &Rc<Self>) {
        if !self.connect_sent.get() {
            return;
        }
        let generation = self.cursor_generation.fetch_add(1) + 1;
        let hc = match self.cursor_buffers.get() {
            Some(cp) => Some(Rc::new(MetalHardwareCursor {
                generation,
                connector: self.clone(),
                cursor_swap_buffer: Cell::new(false),
                cursor_enabled_pending: Cell::new(self.cursor_enabled.get()),
                cursor_x_pending: Cell::new(self.cursor_x.get()),
                cursor_y_pending: Cell::new(self.cursor_y.get()),
                cursor_buffers: cp.clone(),
                have_changes: Cell::new(false),
            }) as _),
            _ => None,
        };
        self.on_change
            .send_event(ConnectorEvent::HardwareCursor(hc));
    }

    fn connected(&self) -> bool {
        let dd = self.display.borrow_mut();
        self.enabled.get()
            && dd.connection == ConnectorStatus::Connected
            && self.primary_plane.get().is_some()
    }

    pub fn schedule_present(&self) {
        self.present_trigger.trigger();
    }

    fn trim_scanout_cache(&self) {
        self.scanout_buffers
            .borrow_mut()
            .retain(|_, buffer| buffer.tex.strong_count() > 0);
    }

    fn prepare_direct_scanout(
        &self,
        pass: &GfxRenderPass,
        plane: &Rc<MetalPlane>,
    ) -> Option<DirectScanoutData> {
        if pass.ops.len() != 1 {
            return None;
        }
        let GfxApiOpt::CopyTexture(ct) = &pass.ops[0] else {
            return None;
        };
        if ct.source != BufferPoints::identity() {
            return None;
        }
        if ct.target.x1 != 0.0
            || ct.target.y1 != 0.0
            || ct.target.x2 != plane.mode_w.get() as f32
            || ct.target.y2 != plane.mode_h.get() as f32
        {
            return None;
        }
        let Some(dmabuf) = ct.tex.dmabuf() else {
            return None;
        };
        let mut cache = self.scanout_buffers.borrow_mut();
        if let Some(buffer) = cache.get(&dmabuf.id) {
            return buffer.fb.as_ref().map(|fb| DirectScanoutData {
                tex: buffer.tex.upgrade().unwrap(),
                fb: fb.clone(),
                dma_buf_id: dmabuf.id,
                acquired: Default::default(),
            });
        }
        let format = 'format: {
            if let Some(f) = plane.formats.get(&dmabuf.format.drm) {
                break 'format f;
            }
            if let Some(opaque) = dmabuf.format.opaque {
                if let Some(f) = plane.formats.get(&opaque.drm) {
                    break 'format f;
                }
            }
            return None;
        };
        if !format.modifiers.contains(&dmabuf.modifier) {
            return None;
        }
        let data = match self.dev.master.add_fb(dmabuf, Some(format.format)) {
            Ok(fb) => Some(DirectScanoutData {
                tex: ct.tex.clone(),
                fb: Rc::new(fb),
                dma_buf_id: dmabuf.id,
                acquired: Default::default(),
            }),
            Err(e) => {
                log::debug!(
                    "Could not import dmabuf for direct scanout: {}",
                    ErrorFmt(e)
                );
                None
            }
        };
        cache.insert(
            dmabuf.id,
            DirectScanoutCache {
                tex: Rc::downgrade(&ct.tex),
                fb: data.as_ref().map(|dsd| dsd.fb.clone()),
            },
        );
        data
    }

    fn direct_scanout_enabled(&self) -> bool {
        self.dev
            .direct_scanout_enabled
            .get()
            .unwrap_or(self.state.direct_scanout_enabled.get())
    }

    fn prepare_present_fb(
        &self,
        rr: &mut RenderResult,
        buffer: &RenderBuffer,
        plane: &Rc<MetalPlane>,
        output: &OutputNode,
        try_direct_scanout: bool,
    ) -> PresentFb {
        self.trim_scanout_cache();
        let buffer_fb = buffer.render_fb();
        let render_hw_cursor = !self.cursor_enabled.get();
        let pass = buffer_fb.create_render_pass(
            output,
            &self.state,
            Some(output.global.pos.get()),
            Some(rr),
            output.global.preferred_scale.get(),
            render_hw_cursor,
            output.has_fullscreen(),
        );
        let try_direct_scanout = try_direct_scanout
            && !output.global.have_shm_screencopies()
            && self.direct_scanout_enabled()
            // at least on AMD, using a FB on a different device for rendering will fail
            // and destroy the render context. it's possible to work around this by waiting
            // until the FB is no longer being scanned out, but if a notification pops up
            // then we must be able to disable direct scanout immediately.
            // https://gitlab.freedesktop.org/drm/amd/-/issues/3186
            && self.dev.is_render_device();
        let mut direct_scanout_data = None;
        if try_direct_scanout {
            if let Some(dsd) = self.prepare_direct_scanout(&pass, plane) {
                output.perform_screencopies(None, &dsd.tex, !render_hw_cursor);
                direct_scanout_data = Some(dsd);
            }
        }
        let direct_scanout_active = direct_scanout_data.is_some();
        if self.direct_scanout_active.replace(direct_scanout_active) != direct_scanout_active {
            let change = match direct_scanout_active {
                true => "Enabling",
                false => "Disabling",
            };
            log::debug!("{} direct scanout on {}", change, self.kernel_id());
        }
        let fb = match &direct_scanout_data {
            None => {
                self.next_buffer.fetch_add(1);
                buffer_fb.perform_render_pass(pass);
                if let Some(tex) = &buffer.dev_tex {
                    buffer.dev_fb.copy_texture(tex, 0, 0);
                }
                output.perform_screencopies(
                    Some(&*buffer_fb),
                    &buffer.render_tex,
                    !render_hw_cursor,
                );
                buffer.drm.clone()
            }
            Some(dsd) => dsd.fb.clone(),
        };
        PresentFb {
            fb,
            direct_scanout_data,
        }
    }

    pub fn present(&self, try_direct_scanout: bool) -> Result<(), ()> {
        let crtc = match self.crtc.get() {
            Some(crtc) => crtc,
            _ => return Ok(()),
        };
        if (!self.has_damage.get() && !self.cursor_changed.get()) || !self.can_present.get() {
            return Ok(());
        }
        if !crtc.active.value.get() {
            return Ok(());
        }
        let plane = match self.primary_plane.get() {
            Some(p) => p,
            _ => return Ok(()),
        };
        let buffers = match self.buffers.get() {
            Some(b) => b,
            _ => return Ok(()),
        };
        let cursor = self.cursor_plane.get();
        let mut new_fb = None;
        let mut changes = self.master.change();
        if self.has_damage.get() {
            if !self.backend.check_render_context(&self.dev) {
                return Ok(());
            }
            if let Some(node) = self.state.root.outputs.get(&self.connector_id) {
                let buffer = &buffers[self.next_buffer.get() % buffers.len()];
                let mut rr = self.render_result.borrow_mut();
                let fb =
                    self.prepare_present_fb(&mut rr, buffer, &plane, &node, try_direct_scanout);
                rr.dispatch_frame_requests();
                changes.change_object(plane.id, |c| {
                    c.change(plane.fb_id, fb.fb.id().0 as _);
                });
                new_fb = Some(fb);
            }
        }
        if self.cursor_changed.get() && cursor.is_some() {
            let plane = cursor.unwrap();
            if self.cursor_enabled.get() {
                let swap_buffer = self.cursor_swap_buffer.take();
                if swap_buffer {
                    self.cursor_front_buffer.fetch_add(1);
                }
                let buffers = self.cursor_buffers.get().unwrap();
                let buffer = &buffers[self.cursor_front_buffer.get() % buffers.len()];
                if swap_buffer {
                    if let Some(tex) = &buffer.dev_tex {
                        buffer.dev_fb.copy_texture(tex, 0, 0);
                    }
                }
                let (width, height) = buffer.dev_fb.size();
                changes.change_object(plane.id, |c| {
                    c.change(plane.fb_id, buffer.drm.id().0 as _);
                    c.change(plane.crtc_id.id, crtc.id.0 as _);
                    c.change(plane.crtc_x.id, self.cursor_x.get() as _);
                    c.change(plane.crtc_y.id, self.cursor_y.get() as _);
                    c.change(plane.crtc_w.id, width as _);
                    c.change(plane.crtc_h.id, height as _);
                    c.change(plane.src_x.id, 0);
                    c.change(plane.src_y.id, 0);
                    c.change(plane.src_w.id, (width as u64) << 16);
                    c.change(plane.src_h.id, (height as u64) << 16);
                });
            } else {
                changes.change_object(plane.id, |c| {
                    c.change(plane.fb_id, 0);
                    c.change(plane.crtc_id.id, 0);
                });
            }
        }
        if let Err(e) = changes.commit(DRM_MODE_ATOMIC_NONBLOCK | DRM_MODE_PAGE_FLIP_EVENT, 0) {
            match e {
                DrmError::Atomic(OsError(c::EACCES)) => {
                    log::debug!("Could not perform atomic commit, likely because we're no longer the DRM master");
                }
                _ => 'handle_failure: {
                    if let Some(fb) = &new_fb {
                        if let Some(dsd) = &fb.direct_scanout_data {
                            if self.present(false).is_ok() {
                                let mut cache = self.scanout_buffers.borrow_mut();
                                if let Some(buffer) = cache.remove(&dsd.dma_buf_id) {
                                    cache.insert(
                                        dsd.dma_buf_id,
                                        DirectScanoutCache {
                                            tex: buffer.tex,
                                            fb: None,
                                        },
                                    );
                                }
                                break 'handle_failure;
                            }
                        }
                    }
                    log::error!("Could not set plane framebuffer: {}", ErrorFmt(e));
                }
            }
            Err(())
        } else {
            if let Some(fb) = new_fb {
                if let Some(dsd) = &fb.direct_scanout_data {
                    dsd.tex.reservations().acquire();
                    dsd.acquired.set(true);
                }
                self.next_framebuffer.set(Some(fb));
            }
            self.can_present.set(false);
            self.has_damage.set(false);
            self.cursor_changed.set(false);
            Ok(())
        }
    }

    pub fn update_drm_feedback(&self) {
        let fb = self.compute_drm_feedback();
        self.drm_feedback.set(fb);
    }

    fn compute_drm_feedback(&self) -> Option<Rc<DrmFeedback>> {
        if !self.dev.is_render_device() {
            return None;
        }
        let default = self.backend.default_feedback.get()?;
        let plane = self.primary_plane.get()?;
        let mut formats = vec![];
        for (format, info) in &plane.formats {
            for modifier in &info.modifiers {
                formats.push((*format, *modifier));
            }
        }
        match default.for_scanout(&self.state.drm_feedback_ids, self.dev.devnum, &formats) {
            Ok(fb) => fb.map(Rc::new),
            Err(e) => {
                log::error!("Could not compute connector feedback: {}", ErrorFmt(e));
                None
            }
        }
    }
}

impl Connector for MetalConnector {
    fn id(&self) -> ConnectorId {
        self.connector_id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        let dd = self.display.borrow_mut();
        ConnectorKernelId {
            ty: dd.connector_type,
            idx: dd.connector_type_id,
        }
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.on_change.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.on_change.set(Some(cb));
    }

    fn damage(&self) {
        self.has_damage.set(true);
        if self.can_present.get() {
            self.schedule_present();
        }
    }

    fn drm_dev(&self) -> Option<DrmDeviceId> {
        Some(self.dev.id)
    }

    fn set_enabled(&self, enabled: bool) {
        if self.enabled.replace(enabled) != enabled {
            if self.display.borrow_mut().connection == ConnectorStatus::Connected {
                if let Some(dev) = self.backend.device_holder.drm_devices.get(&self.dev.devnum) {
                    if let Err(e) = self.backend.handle_drm_change_(&dev, true) {
                        dev.unprocessed_change.set(true);
                        log::error!("Could not dis/enable connector: {}", ErrorFmt(e));
                    }
                }
            }
        }
    }

    fn drm_feedback(&self) -> Option<Rc<DrmFeedback>> {
        self.drm_feedback.get()
    }
}

#[derive(Debug)]
pub struct MetalCrtc {
    pub id: DrmCrtc,
    pub idx: usize,
    pub master: Rc<DrmMaster>,

    pub possible_planes: AHashMap<DrmPlane, Rc<MetalPlane>>,

    pub connector: CloneCell<Option<Rc<MetalConnector>>>,

    pub active: MutableProperty<bool>,
    pub mode_id: MutableProperty<DrmBlob>,
    pub out_fence_ptr: DrmProperty,

    pub mode_blob: CloneCell<Option<Rc<PropBlob>>>,
}

#[derive(Debug)]
pub struct MetalEncoder {
    pub id: DrmEncoder,
    pub crtcs: AHashMap<DrmCrtc, Rc<MetalCrtc>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PlaneType {
    Overlay,
    Primary,
    Cursor,
}

#[derive(Debug)]
pub struct PlaneFormat {
    format: &'static Format,
    modifiers: IndexSet<Modifier>,
}

#[derive(Debug)]
pub struct MetalPlane {
    pub id: DrmPlane,
    pub master: Rc<DrmMaster>,

    pub ty: PlaneType,

    pub possible_crtcs: u32,
    pub formats: AHashMap<u32, PlaneFormat>,

    pub assigned: Cell<bool>,

    pub mode_w: Cell<i32>,
    pub mode_h: Cell<i32>,

    pub crtc_id: MutableProperty<DrmCrtc>,
    pub crtc_x: MutableProperty<i32>,
    pub crtc_y: MutableProperty<i32>,
    pub crtc_w: MutableProperty<i32>,
    pub crtc_h: MutableProperty<i32>,
    pub src_x: MutableProperty<u32>,
    pub src_y: MutableProperty<u32>,
    pub src_w: MutableProperty<u32>,
    pub src_h: MutableProperty<u32>,
    pub in_fence_fd: DrmProperty,
    pub fb_id: DrmProperty,
}

fn get_connectors(
    backend: &Rc<MetalBackend>,
    dev: &Rc<MetalDrmDevice>,
    ids: &[DrmConnector],
) -> Result<
    (
        CopyHashMap<DrmConnector, Rc<MetalConnector>>,
        CopyHashMap<DrmConnector, ConnectorFutures>,
    ),
    DrmError,
> {
    let connectors = CopyHashMap::new();
    let futures = CopyHashMap::new();
    for connector in ids {
        match create_connector(backend, *connector, dev) {
            Ok((con, fut)) => {
                let id = con.id;
                connectors.set(id, con);
                futures.set(id, fut);
            }
            Err(e) => return Err(DrmError::CreateConnector(Box::new(e))),
        }
    }
    Ok((connectors, futures))
}

fn create_connector(
    backend: &Rc<MetalBackend>,
    connector: DrmConnector,
    dev: &Rc<MetalDrmDevice>,
) -> Result<(Rc<MetalConnector>, ConnectorFutures), DrmError> {
    let display = create_connector_display_data(connector, dev)?;
    let slf = Rc::new(MetalConnector {
        id: connector,
        master: dev.master.clone(),
        state: backend.state.clone(),
        dev: dev.clone(),
        backend: backend.clone(),
        connector_id: backend.state.connector_ids.next(),
        buffers: Default::default(),
        next_buffer: Default::default(),
        enabled: Cell::new(true),
        can_present: Cell::new(true),
        has_damage: Cell::new(true),
        primary_plane: Default::default(),
        cursor_plane: Default::default(),
        crtc: Default::default(),
        on_change: Default::default(),
        present_trigger: Default::default(),
        render_result: RefCell::new(Default::default()),
        cursor_generation: Default::default(),
        cursor_x: Cell::new(0),
        cursor_y: Cell::new(0),
        cursor_enabled: Cell::new(false),
        cursor_buffers: Default::default(),
        display: RefCell::new(display),
        connect_sent: Cell::new(false),
        cursor_changed: Cell::new(false),
        cursor_front_buffer: Default::default(),
        cursor_swap_buffer: Cell::new(false),
        drm_feedback: Default::default(),
        scanout_buffers: Default::default(),
        active_framebuffer: Default::default(),
        next_framebuffer: Default::default(),
        direct_scanout_active: Cell::new(false),
    });
    let futures = ConnectorFutures {
        present: backend
            .state
            .eng
            .spawn2(Phase::Present, slf.clone().present_loop()),
    };
    Ok((slf, futures))
}

fn create_connector_display_data(
    connector: DrmConnector,
    dev: &Rc<MetalDrmDevice>,
) -> Result<ConnectorDisplayData, DrmError> {
    let info = dev.master.get_connector_info(connector, true)?;
    let mut crtcs = AHashMap::new();
    for encoder in info.encoders {
        if let Some(encoder) = dev.encoders.get(&encoder) {
            for (_, crtc) in &encoder.crtcs {
                crtcs.insert(crtc.id, crtc.clone());
            }
        }
    }
    let props = collect_properties(&dev.master, connector)?;
    let connection = ConnectorStatus::from_drm(info.connection);
    let mut name = String::new();
    let mut manufacturer = String::new();
    let mut serial_number = String::new();
    let mode = info.modes.first().cloned().map(Rc::new);
    let refresh = mode
        .as_ref()
        .map(|m| 1_000_000_000_000u64 / (m.refresh_rate_millihz() as u64))
        .unwrap_or(0) as u32;
    let connector_type = ConnectorType::from_drm(info.connector_type);
    let connector_name = debug_fn(|f| write!(f, "{}-{}", connector_type, info.connector_type_id));
    'fetch_edid: {
        if connection != ConnectorStatus::Connected {
            break 'fetch_edid;
        }
        let edid = match props.get("EDID") {
            Ok(e) => e,
            _ => {
                log::warn!(
                    "Connector {} is connected but has no EDID blob",
                    connector_name,
                );
                break 'fetch_edid;
            }
        };
        let blob = match dev.master.getblob_vec::<u8>(DrmBlob(edid.value.get() as _)) {
            Ok(b) => b,
            Err(e) => {
                log::error!(
                    "Could not fetch edid property of connector {}: {}",
                    connector_name,
                    ErrorFmt(e)
                );
                break 'fetch_edid;
            }
        };
        let edid = match crate::edid::parse(&blob) {
            Ok(e) => e,
            Err(e) => {
                log::error!(
                    "Could not parse edid property of connector {}: {}",
                    connector_name,
                    ErrorFmt(e)
                );
                break 'fetch_edid;
            }
        };
        manufacturer = edid.base_block.id_manufacturer_name.to_string();
        for descriptor in edid.base_block.descriptors.iter().flatten() {
            match descriptor {
                Descriptor::DisplayProductSerialNumber(s) => {
                    serial_number = s.clone();
                }
                Descriptor::DisplayProductName(s) => {
                    name = s.clone();
                }
                _ => {}
            }
        }
        if name.is_empty() {
            log::warn!(
                "The display attached to connector {} does not have a product name descriptor",
                connector_name,
            );
        }
        if serial_number.is_empty() {
            log::warn!(
                "The display attached to connector {} does not have a serial number descriptor",
                connector_name,
            );
            serial_number = edid.base_block.id_serial_number.to_string();
        }
    }
    let props = collect_properties(&dev.master, connector)?;
    let connector_type = ConnectorType::from_drm(info.connector_type);
    Ok(ConnectorDisplayData {
        crtc_id: props.get("CRTC_ID")?.map(|v| DrmCrtc(v as _)),
        crtcs,
        modes: info.modes,
        mode,
        refresh,
        monitor_manufacturer: manufacturer,
        monitor_name: name,
        monitor_serial_number: serial_number,
        connection,
        mm_width: info.mm_width,
        mm_height: info.mm_height,
        subpixel: info.subpixel,
        connector_type,
        connector_type_id: info.connector_type_id,
    })
}

fn create_encoder(
    encoder: DrmEncoder,
    master: &Rc<DrmMaster>,
    crtcs: &AHashMap<DrmCrtc, Rc<MetalCrtc>>,
) -> Result<MetalEncoder, DrmError> {
    let info = master.get_encoder_info(encoder)?;
    let mut possible = AHashMap::new();
    for crtc in crtcs.values() {
        if info.possible_crtcs.contains(1 << crtc.idx) {
            possible.insert(crtc.id, crtc.clone());
        }
    }
    Ok(MetalEncoder {
        id: encoder,
        crtcs: possible,
    })
}

fn create_crtc(
    crtc: DrmCrtc,
    idx: usize,
    master: &Rc<DrmMaster>,
    planes: &AHashMap<DrmPlane, Rc<MetalPlane>>,
) -> Result<MetalCrtc, DrmError> {
    let mask = 1 << idx;
    let mut possible_planes = AHashMap::new();
    for plane in planes.values() {
        if plane.possible_crtcs.contains(mask) {
            possible_planes.insert(plane.id, plane.clone());
        }
    }
    let props = collect_properties(master, crtc)?;
    Ok(MetalCrtc {
        id: crtc,
        idx,
        master: master.clone(),
        possible_planes,
        connector: Default::default(),
        active: props.get("ACTIVE")?.map(|v| v == 1),
        mode_id: props.get("MODE_ID")?.map(|v| DrmBlob(v as u32)),
        out_fence_ptr: props.get("OUT_FENCE_PTR")?.id,
        mode_blob: Default::default(),
    })
}

fn create_plane(plane: DrmPlane, master: &Rc<DrmMaster>) -> Result<MetalPlane, DrmError> {
    let info = master.get_plane_info(plane)?;
    let props = collect_properties(master, plane)?;
    let mut formats = AHashMap::new();
    if let Some((_, v)) = props.props.get(b"IN_FORMATS".as_bstr()) {
        for format in master.get_in_formats(*v as _)? {
            if format.modifiers.is_empty() {
                continue;
            }
            if let Some(f) = crate::format::formats().get(&format.format) {
                formats.insert(
                    format.format,
                    PlaneFormat {
                        format: f,
                        modifiers: format.modifiers,
                    },
                );
            }
        }
    } else {
        for format in info.format_types {
            if let Some(f) = crate::format::formats().get(&format) {
                formats.insert(
                    format,
                    PlaneFormat {
                        format: f,
                        modifiers: indexset![INVALID_MODIFIER],
                    },
                );
            }
        }
    }
    let ty = match props.props.get(b"type".as_bstr()) {
        Some((def, val)) => match &def.ty {
            DrmPropertyType::Enum { values, .. } => 'ty: {
                for v in values {
                    if v.value == *val {
                        match v.name.as_bytes() {
                            b"Overlay" => break 'ty PlaneType::Overlay,
                            b"Primary" => break 'ty PlaneType::Primary,
                            b"Cursor" => break 'ty PlaneType::Cursor,
                            _ => return Err(DrmError::UnknownPlaneType(v.name.to_owned())),
                        }
                    }
                }
                return Err(DrmError::InvalidPlaneType(*val));
            }
            _ => return Err(DrmError::InvalidPlaneTypeProperty),
        },
        _ => {
            return Err(DrmError::MissingProperty(
                "type".to_string().into_boxed_str(),
            ))
        }
    };
    Ok(MetalPlane {
        id: plane,
        master: master.clone(),
        ty,
        possible_crtcs: info.possible_crtcs,
        formats,
        fb_id: props.get("FB_ID")?.id,
        crtc_id: props.get("CRTC_ID")?.map(|v| DrmCrtc(v as _)),
        crtc_x: props.get("CRTC_X")?.map(|v| v as i32),
        crtc_y: props.get("CRTC_Y")?.map(|v| v as i32),
        crtc_w: props.get("CRTC_W")?.map(|v| v as i32),
        crtc_h: props.get("CRTC_H")?.map(|v| v as i32),
        src_x: props.get("SRC_X")?.map(|v| v as u32),
        src_y: props.get("SRC_Y")?.map(|v| v as u32),
        src_w: props.get("SRC_W")?.map(|v| v as u32),
        src_h: props.get("SRC_H")?.map(|v| v as u32),
        in_fence_fd: props.get("IN_FENCE_FD")?.id,
        assigned: Cell::new(false),
        mode_w: Cell::new(0),
        mode_h: Cell::new(0),
    })
}

fn collect_properties<T: DrmObject>(
    master: &Rc<DrmMaster>,
    t: T,
) -> Result<CollectedProperties, DrmError> {
    let mut props = AHashMap::new();
    for prop in master.get_properties(t)? {
        let def = master.get_property(prop.id)?;
        props.insert(def.name.clone(), (def, prop.value));
    }
    Ok(CollectedProperties { props })
}

fn collect_untyped_properties<T: DrmObject>(
    master: &Rc<DrmMaster>,
    t: T,
) -> Result<AHashMap<DrmProperty, u64>, DrmError> {
    let mut props = AHashMap::new();
    for prop in master.get_properties(t)? {
        props.insert(prop.id, prop.value);
    }
    Ok(props)
}

struct CollectedProperties {
    props: AHashMap<BString, (DrmPropertyDefinition, u64)>,
}

impl CollectedProperties {
    fn get(&self, name: &str) -> Result<MutableProperty<u64>, DrmError> {
        match self.props.get(name.as_bytes().as_bstr()) {
            Some((def, value)) => Ok(MutableProperty {
                id: def.id,
                value: Cell::new(*value),
            }),
            _ => Err(DrmError::MissingProperty(name.to_string().into_boxed_str())),
        }
    }
}

#[derive(Debug)]
pub struct MutableProperty<T: Copy> {
    pub id: DrmProperty,
    pub value: Cell<T>,
}

impl<T: Copy> MutableProperty<T> {
    fn map<U: Copy, F>(self, f: F) -> MutableProperty<U>
    where
        F: FnOnce(T) -> U,
    {
        MutableProperty {
            id: self.id,
            value: Cell::new(f(self.value.into_inner())),
        }
    }
}

#[derive(Default)]
struct Preserve {
    connectors: AHashSet<DrmConnector>,
    crtcs: AHashSet<DrmCrtc>,
    planes: AHashSet<DrmPlane>,
}

impl MetalBackend {
    fn check_render_context(&self, dev: &Rc<MetalDrmDevice>) -> bool {
        let ctx = match self.ctx.get() {
            Some(ctx) => ctx,
            None => return false,
        };
        if let Some(r) = ctx
            .gfx
            .reset_status()
            .or_else(|| dev.ctx.get().gfx.reset_status())
        {
            fatal!("EGL context has been reset: {:?}", r);
        }
        true
    }

    // fn check_render_context(&self) -> bool {
    //     let ctx = match self.ctx.get() {
    //         Some(ctx) => ctx,
    //         None => return false,
    //     };
    //     let reset = match ctx.egl.reset_status() {
    //         Some(r) => r,
    //         None => return true,
    //     };
    //     log::error!("EGL context has been reset: {:?}", reset);
    //     if reset != ResetStatus::Innocent {
    //         fatal!("We are not innocent. Terminating.");
    //     }
    //     log::info!("Trying to create a new context");
    //     self.ctx.set(None);
    //     self.state.set_render_ctx(None);
    //     let mut old_buffers = vec![];
    //     let mut ctx_dev = None;
    //     for dev in self.device_holder.drm_devices.lock().values() {
    //         if dev.dev.id == ctx.dev_id {
    //             ctx_dev = Some(dev.dev.clone());
    //         }
    //         for connector in dev.connectors.lock().values() {
    //             old_buffers.push(connector.buffers.take());
    //         }
    //     }
    //     if let Some(dev) = &ctx_dev {
    //         self.make_render_device(dev, true)
    //     } else {
    //         false
    //     }
    // }

    pub fn handle_drm_change(self: &Rc<Self>, dev: UdevDevice) -> Option<()> {
        let dev = match self.device_holder.drm_devices.get(&dev.devnum()) {
            Some(dev) => dev,
            _ => return None,
        };
        if let Err(e) = self.handle_drm_change_(&dev, true) {
            dev.unprocessed_change.set(true);
            log::error!("Could not handle change of drm device: {}", ErrorFmt(e));
        }
        None
    }

    fn handle_drm_change_(
        self: &Rc<Self>,
        dev: &Rc<MetalDrmDeviceData>,
        preserve_any: bool,
    ) -> Result<(), MetalError> {
        if let Err(e) = self.update_device_properties(dev) {
            return Err(MetalError::UpdateProperties(e));
        }
        let res = dev.dev.master.get_resources()?;
        let current_connectors: AHashSet<_> = res.connectors.iter().copied().collect();
        let mut new_connectors = AHashSet::new();
        let mut removed_connectors = AHashSet::new();
        for c in &res.connectors {
            if !dev.connectors.contains(c) {
                new_connectors.insert(*c);
            }
        }
        for c in dev.connectors.lock().keys() {
            if !current_connectors.contains(c) {
                removed_connectors.insert(*c);
            }
        }
        for c in removed_connectors {
            dev.futures.remove(&c);
            if let Some(c) = dev.connectors.remove(&c) {
                if c.connect_sent.get() {
                    c.on_change.send_event(ConnectorEvent::Disconnected);
                }
                c.on_change.send_event(ConnectorEvent::Removed);
            }
        }
        let mut preserve = Preserve::default();
        for c in dev.connectors.lock().values() {
            let mut dd = match create_connector_display_data(c.id, &dev.dev) {
                Ok(d) => d,
                Err(e) => {
                    log::error!(
                        "Could not update display data for connector: {}",
                        ErrorFmt(e)
                    );
                    continue;
                }
            };
            let mut old = c.display.borrow_mut();
            mem::swap(old.deref_mut(), &mut dd);
            if c.connect_sent.get() {
                if !c.enabled.get()
                    || old.connection != ConnectorStatus::Connected
                    || !old.is_same_monitor(&dd)
                {
                    c.on_change.send_event(ConnectorEvent::Disconnected);
                    c.connect_sent.set(false);
                } else if preserve_any {
                    preserve.connectors.insert(c.id);
                }
            }
        }
        for c in new_connectors {
            let (connector, future) = match create_connector(self, c, &dev.dev) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Could not create new drm connector: {}", ErrorFmt(e));
                    continue;
                }
            };
            self.state
                .backend_events
                .push(BackendEvent::NewConnector(connector.clone()));
            dev.futures.set(c, future);
            dev.connectors.set(c, connector);
        }
        self.init_drm_device(dev, &mut preserve)?;
        for connector in dev.connectors.lock().values() {
            if connector.connected() {
                if !preserve.connectors.contains(&connector.id) {
                    connector.can_present.set(true);
                }
                self.start_connector(connector, true);
            }
        }
        dev.unprocessed_change.set(false);
        Ok(())
    }

    fn send_connected(&self, connector: &Rc<MetalConnector>, dd: &ConnectorDisplayData) {
        let mut prev_mode = None;
        let mut modes = vec![];
        for mode in dd.modes.iter().map(|m| m.to_backend()) {
            if prev_mode.replace(mode) != Some(mode) {
                modes.push(mode);
            }
        }
        connector
            .on_change
            .send_event(ConnectorEvent::Connected(MonitorInfo {
                modes,
                manufacturer: dd.monitor_manufacturer.clone(),
                product: dd.monitor_name.clone(),
                serial_number: dd.monitor_serial_number.clone(),
                initial_mode: dd.mode.clone().unwrap().to_backend(),
                width_mm: dd.mm_width as _,
                height_mm: dd.mm_height as _,
            }));
        connector.connect_sent.set(true);
        connector.send_hardware_cursor();
    }

    pub fn create_drm_device(
        self: &Rc<Self>,
        pending: PendingDrmDevice,
        master: &Rc<DrmMaster>,
    ) -> Result<Rc<MetalDrmDeviceData>, MetalError> {
        if let Err(e) = master.set_client_cap(DRM_CLIENT_CAP_ATOMIC, 2) {
            return Err(MetalError::AtomicModesetting(e));
        }
        let resources = master.get_resources()?;

        let (cursor_width, cursor_height) = match master.get_cursor_size() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Can't determine size of cursor planes: {}", ErrorFmt(e));
                (64, 64)
            }
        };

        let mut planes = AHashMap::new();
        for plane in master.get_planes()? {
            match create_plane(plane, master) {
                Ok(p) => {
                    planes.insert(p.id, Rc::new(p));
                }
                Err(e) => return Err(MetalError::CreatePlane(e)),
            }
        }

        let mut crtcs = AHashMap::new();
        for (idx, crtc) in resources.crtcs.iter().copied().enumerate() {
            match create_crtc(crtc, idx, master, &planes) {
                Ok(c) => {
                    crtcs.insert(c.id, Rc::new(c));
                }
                Err(e) => return Err(MetalError::CreateCrtc(e)),
            }
        }

        let mut encoders = AHashMap::new();
        for encoder in resources.encoders {
            match create_encoder(encoder, master, &crtcs) {
                Ok(e) => {
                    encoders.insert(e.id, Rc::new(e));
                }
                Err(e) => return Err(MetalError::CreateEncoder(e)),
            }
        }

        let gfx = match self.state.create_gfx_context(master, None) {
            Ok(r) => r,
            Err(e) => return Err(MetalError::CreateRenderContex(e)),
        };
        let ctx = Rc::new(MetalRenderContext {
            dev_id: pending.id,
            gfx,
        });

        let gbm = match GbmDevice::new(master) {
            Ok(g) => g,
            Err(e) => return Err(MetalError::GbmDevice(e)),
        };

        let dev = Rc::new(MetalDrmDevice {
            backend: self.clone(),
            id: pending.id,
            devnum: pending.devnum,
            devnode: pending.devnode,
            master: master.clone(),
            crtcs,
            encoders,
            planes,
            min_width: resources.min_width,
            max_width: resources.max_width,
            min_height: resources.min_height,
            max_height: resources.max_height,
            cursor_width,
            cursor_height,
            gbm,
            handle_events: HandleEvents {
                handle_events: Cell::new(None),
            },
            ctx: CloneCell::new(ctx),
            on_change: Default::default(),
            direct_scanout_enabled: Default::default(),
        });

        let (connectors, futures) = get_connectors(self, &dev, &resources.connectors)?;

        let slf = Rc::new(MetalDrmDeviceData {
            dev: dev.clone(),
            connectors,
            futures,
            unprocessed_change: Cell::new(false),
        });

        self.init_drm_device(&slf, &mut Preserve::default())?;

        self.state
            .backend_events
            .push(BackendEvent::NewDrmDevice(dev.clone()));

        for connector in slf.connectors.lock().values() {
            self.state
                .backend_events
                .push(BackendEvent::NewConnector(connector.clone()));
            if connector.connected() {
                self.start_connector(connector, true);
            }
        }

        let drm_handler = self
            .state
            .eng
            .spawn(self.clone().handle_drm_events(slf.clone()));
        slf.dev.handle_events.handle_events.set(Some(drm_handler));

        Ok(slf)
    }

    fn update_device_properties(&self, dev: &Rc<MetalDrmDeviceData>) -> Result<(), DrmError> {
        let get = |p: &AHashMap<DrmProperty, _>, k: DrmProperty| match p.get(&k) {
            Some(v) => Ok(*v),
            _ => todo!(),
        };
        let master = &dev.dev.master;
        for c in dev.connectors.lock().values() {
            let dd = c.display.borrow_mut();
            let props = collect_untyped_properties(master, c.id)?;
            dd.crtc_id
                .value
                .set(DrmCrtc(get(&props, dd.crtc_id.id)? as _));
        }
        for c in dev.dev.crtcs.values() {
            let props = collect_untyped_properties(master, c.id)?;
            c.active.value.set(get(&props, c.active.id)? != 0);
            c.mode_id
                .value
                .set(DrmBlob(get(&props, c.mode_id.id)? as _));
        }
        for c in dev.dev.planes.values() {
            let props = collect_untyped_properties(master, c.id)?;
            c.crtc_id
                .value
                .set(DrmCrtc(get(&props, c.crtc_id.id)? as _));
        }
        Ok(())
    }

    pub fn resume_drm_device(
        self: &Rc<Self>,
        dev: &Rc<MetalDrmDeviceData>,
    ) -> Result<(), MetalError> {
        for connector in dev.connectors.lock().values() {
            connector.can_present.set(true);
            connector.has_damage.set(true);
            connector.cursor_changed.set(true);
        }
        if dev.unprocessed_change.get() {
            return self.handle_drm_change_(dev, false);
        }
        if let Err(e) = self.update_device_properties(dev) {
            return Err(MetalError::UpdateProperties(e));
        }
        let mut preserve = Preserve::default();
        self.init_drm_device(dev, &mut preserve)?;
        for connector in dev.connectors.lock().values() {
            if connector.primary_plane.get().is_some() {
                connector.schedule_present();
            }
        }
        Ok(())
    }

    async fn handle_drm_events(self: Rc<Self>, dev: Rc<MetalDrmDeviceData>) {
        loop {
            match dev.dev.master.event().await {
                Ok(Some(e)) => self.handle_drm_event(e, &dev),
                Ok(None) => break,
                Err(e) => {
                    log::error!("Could not read DRM event: {}", ErrorFmt(e));
                    return;
                }
            }
        }
    }

    fn handle_drm_event(self: &Rc<Self>, event: DrmEvent, dev: &Rc<MetalDrmDeviceData>) {
        match event {
            DrmEvent::FlipComplete {
                tv_sec,
                tv_usec,
                sequence,
                crtc_id,
            } => self.handle_drm_flip_event(dev, crtc_id, tv_sec, tv_usec, sequence),
        }
    }

    fn handle_drm_flip_event(
        self: &Rc<Self>,
        dev: &Rc<MetalDrmDeviceData>,
        crtc_id: DrmCrtc,
        tv_sec: u32,
        tv_usec: u32,
        sequence: u32,
    ) {
        let crtc = match dev.dev.crtcs.get(&crtc_id) {
            Some(c) => c,
            _ => return,
        };
        let connector = match crtc.connector.get() {
            Some(c) => c,
            _ => return,
        };
        connector.can_present.set(true);
        connector
            .active_framebuffer
            .set(connector.next_framebuffer.take());
        if connector.has_damage.get() || connector.cursor_changed.get() {
            connector.schedule_present();
        }
        let dd = connector.display.borrow_mut();
        {
            let global = self.state.outputs.get(&connector.connector_id);
            let mut rr = connector.render_result.borrow_mut();
            if let Some(g) = &global {
                let refresh = dd.refresh;
                let bindings = g.node.global.bindings.borrow_mut();
                for fb in rr.presentation_feedbacks.drain(..) {
                    if let Some(bindings) = bindings.get(&fb.client.id) {
                        for binding in bindings.values() {
                            fb.send_sync_output(binding);
                        }
                    }
                    fb.send_presented(
                        tv_sec as _,
                        tv_usec * 1000,
                        refresh,
                        sequence as _,
                        KIND_VSYNC | KIND_HW_COMPLETION,
                    );
                    let _ = fb.client.remove_obj(&*fb);
                }
            } else {
                for fb in rr.presentation_feedbacks.drain(..) {
                    fb.send_discarded();
                    let _ = fb.client.remove_obj(&*fb);
                }
            }
        }
    }

    fn reset_planes(&self, dev: &MetalDrmDeviceData, changes: &mut Change, preserve: &Preserve) {
        for plane in dev.dev.planes.values() {
            if preserve.planes.contains(&plane.id) {
                continue;
            }
            plane.crtc_id.value.set(DrmCrtc::NONE);
            plane.assigned.set(false);
            changes.change_object(plane.id, |c| {
                c.change(plane.crtc_id.id, 0);
                c.change(plane.fb_id, 0);
                c.change(plane.in_fence_fd, -1i32 as u64);
            })
        }
    }

    fn reset_connectors_and_crtcs(
        &self,
        dev: &MetalDrmDeviceData,
        changes: &mut Change,
        preserve: &Preserve,
    ) {
        for connector in dev.connectors.lock().values() {
            if preserve.connectors.contains(&connector.id) {
                continue;
            }
            connector.primary_plane.set(None);
            connector.cursor_plane.set(None);
            connector.cursor_enabled.set(false);
            connector.crtc.set(None);
            let dd = connector.display.borrow_mut();
            dd.crtc_id.value.set(DrmCrtc::NONE);
            changes.change_object(connector.id, |c| {
                c.change(dd.crtc_id.id, 0);
            })
        }
        for crtc in dev.dev.crtcs.values() {
            if preserve.crtcs.contains(&crtc.id) {
                continue;
            }
            crtc.connector.set(None);
            crtc.active.value.set(false);
            crtc.mode_id.value.set(DrmBlob::NONE);
            changes.change_object(crtc.id, |c| {
                c.change(crtc.active.id, 0);
                c.change(crtc.mode_id.id, 0);
                c.change(crtc.out_fence_ptr, 0);
            })
        }
    }

    fn validate_preserve(&self, dev: &Rc<MetalDrmDeviceData>, preserve: &mut Preserve) {
        let mut remove_connectors = vec![];
        macro_rules! fail {
            ($c:expr) => {{
                remove_connectors.push($c);
                continue;
            }};
        }
        for c in &preserve.connectors {
            let c = match dev.connectors.get(c) {
                Some(c) => c,
                _ => {
                    log::warn!("Cannot preserve connector which no longer exists");
                    fail!(*c)
                }
            };
            let dd = c.display.borrow_mut();
            if let Some(crtc) = c.crtc.get() {
                if dd.crtc_id.value.get() != crtc.id {
                    log::warn!("Cannot preserve connector attached to a different crtc");
                    fail!(c.id);
                }
                if let Some(mode) = &dd.mode {
                    let mode_id = crtc.mode_id.value.get();
                    if mode_id.is_none() {
                        log::warn!("Cannot preserve connector whose crtc has no mode attached");
                        fail!(c.id);
                    }
                    let current_mode = match dev.dev.master.getblob::<drm_mode_modeinfo>(mode_id) {
                        Ok(m) => m.into(),
                        _ => {
                            log::warn!("Could not retrieve current mode of connector");
                            fail!(c.id);
                        }
                    };
                    if !modes_equal(mode, &current_mode) {
                        log::warn!("Cannot preserve connector whose crtc has a different mode");
                        fail!(c.id);
                    }
                }
                if !crtc.active.value.get() {
                    log::warn!("Cannot preserve connector whose crtc is inactive");
                    fail!(c.id);
                }
                if let Some(plane) = c.primary_plane.get() {
                    if plane.crtc_id.value.get() != crtc.id {
                        log::warn!("Cannot preserve connector whose primary plane is attached to a different crtc");
                        fail!(c.id);
                    }
                }
                if let Some(plane) = c.cursor_plane.get() {
                    let crtc_id = plane.crtc_id.value.get();
                    if crtc_id.is_some() && crtc_id != crtc.id {
                        log::warn!("Cannot preserve connector whose cursor plane is attached to a different crtc");
                        fail!(c.id);
                    }
                }
            }
        }
        for c in remove_connectors {
            preserve.connectors.remove(&c);
        }
        for connector in dev.connectors.lock().values() {
            if preserve.connectors.contains(&connector.id) {
                if let Some(pp) = connector.primary_plane.get() {
                    preserve.planes.insert(pp.id);
                }
                if let Some(pp) = connector.cursor_plane.get() {
                    preserve.planes.insert(pp.id);
                }
                if let Some(crtc) = connector.crtc.get() {
                    preserve.crtcs.insert(crtc.id);
                }
            }
        }
    }

    fn make_render_device(&self, dev: &MetalDrmDevice, force: bool) {
        if !force {
            if let Some(ctx) = self.ctx.get() {
                if ctx.dev_id == dev.id {
                    return;
                }
            }
        }
        let ctx = dev.ctx.get();
        self.state.set_render_ctx(Some(ctx.gfx.clone()));
        let fb = match DrmFeedback::new(&self.state.drm_feedback_ids, &*ctx.gfx) {
            Ok(fb) => Some(Rc::new(fb)),
            Err(e) => {
                log::error!("Could not create feedback for new context: {}", ErrorFmt(e));
                None
            }
        };
        self.default_feedback.set(fb);
        self.ctx.set(Some(ctx));
        for dev in self.device_holder.drm_devices.lock().values() {
            self.re_init_drm_device(&dev);
        }
    }

    fn set_gfx_api(&self, dev: &MetalDrmDevice, api: GfxApi) {
        if dev.ctx.get().gfx.gfx_api() == api {
            return;
        }
        let gfx = match self.state.create_gfx_context(&dev.master, Some(api)) {
            Ok(r) => r,
            Err(e) => {
                log::error!(
                    "Could not create a new graphics context for device {:?}: {}",
                    dev.devnode,
                    ErrorFmt(e)
                );
                return;
            }
        };
        dev.on_change
            .send_event(crate::backend::DrmEvent::GfxApiChanged);
        dev.ctx.set(Rc::new(MetalRenderContext {
            dev_id: dev.id,
            gfx,
        }));
        if dev.is_render_device() {
            self.make_render_device(dev, true);
        } else {
            if let Some(dev) = self.device_holder.drm_devices.get(&dev.devnum) {
                self.re_init_drm_device(&dev);
            }
        }
    }

    fn re_init_drm_device(&self, dev: &Rc<MetalDrmDeviceData>) {
        let mut preserve = Preserve::default();
        if let Err(e) = self.init_drm_device(dev, &mut preserve) {
            log::error!("Could not initialize device: {}", ErrorFmt(e));
        }
        for connector in dev.connectors.lock().values() {
            if connector.connected() {
                self.start_connector(connector, false);
            }
        }
    }

    fn init_drm_device(
        &self,
        dev: &Rc<MetalDrmDeviceData>,
        preserve: &mut Preserve,
    ) -> Result<(), MetalError> {
        let ctx = match self.ctx.get() {
            Some(ctx) => ctx,
            _ => return Ok(()),
        };
        self.validate_preserve(dev, preserve);
        let mut flags = 0;
        let mut changes = dev.dev.master.change();
        if !self.can_use_current_drm_mode(dev) {
            log::warn!("Cannot use existing connector configuration. Trying to perform modeset.");
            flags = DRM_MODE_ATOMIC_ALLOW_MODESET;
            self.reset_connectors_and_crtcs(dev, &mut changes, preserve);
            for connector in dev.connectors.lock().values() {
                if !preserve.connectors.contains(&connector.id) {
                    if let Err(e) = self.assign_connector_crtc(connector, &mut changes) {
                        log::error!("Could not assign a crtc: {}", ErrorFmt(e));
                    }
                }
            }
        }
        self.reset_planes(dev, &mut changes, preserve);
        let mut old_buffers = vec![];
        for connector in dev.connectors.lock().values() {
            if !preserve.connectors.contains(&connector.id) {
                if let Err(e) =
                    self.assign_connector_planes(connector, &mut changes, &ctx, &mut old_buffers)
                {
                    log::error!("Could not assign a plane: {}", ErrorFmt(e));
                }
            }
        }
        if let Err(e) = changes.commit(flags, 0) {
            return Err(MetalError::Modeset(e));
        }
        for connector in dev.connectors.lock().values() {
            if preserve.connectors.contains(&connector.id) {
                continue;
            }
            connector.send_hardware_cursor();
            connector.update_drm_feedback();
        }
        Ok(())
    }

    fn can_use_current_drm_mode(&self, dev: &Rc<MetalDrmDeviceData>) -> bool {
        let mut used_crtcs = AHashSet::new();
        let mut used_planes = AHashSet::new();

        for connector in dev.connectors.lock().values() {
            let dd = connector.display.borrow_mut();
            if !connector.enabled.get() || dd.connection != ConnectorStatus::Connected {
                if dd.crtc_id.value.get().is_some() {
                    log::debug!("Connector is not connected but has an assigned crtc");
                    return false;
                }
                continue;
            }
            let crtc_id = dd.crtc_id.value.get();
            if crtc_id.is_none() {
                log::debug!("Connector is connected but has no assigned crtc");
                return false;
            }
            used_crtcs.insert(crtc_id);
            let crtc = dev.dev.crtcs.get(&crtc_id).unwrap();
            connector.crtc.set(Some(crtc.clone()));
            crtc.connector.set(Some(connector.clone()));
            if !crtc.active.value.get() {
                log::debug!("Crtc is not active");
                return false;
            }
            let mode = match &dd.mode {
                Some(m) => m,
                _ => {
                    log::debug!("Connector has no assigned mode");
                    return false;
                }
            };
            let current_mode = match dev
                .dev
                .master
                .getblob::<drm_mode_modeinfo>(crtc.mode_id.value.get())
            {
                Ok(m) => m.into(),
                _ => {
                    log::debug!("Could not retrieve current mode of connector");
                    return false;
                }
            };
            if !modes_equal(mode, &current_mode) {
                log::debug!("Connector mode differs from desired mode");
                return false;
            }
            let mut have_primary_plane = false;
            for plane in crtc.possible_planes.values() {
                if plane.ty == PlaneType::Primary && used_planes.insert(plane.id) {
                    have_primary_plane = true;
                    break;
                }
            }
            if !have_primary_plane {
                log::debug!("Connector has no primary plane assigned");
                return false;
            }
        }

        let mut changes = dev.dev.master.change();
        let mut flags = 0;
        for crtc in dev.dev.crtcs.values() {
            changes.change_object(crtc.id, |c| {
                if !used_crtcs.contains(&crtc.id) && crtc.active.value.take() {
                    flags |= DRM_MODE_ATOMIC_ALLOW_MODESET;
                    c.change(crtc.active.id, 0);
                }
                c.change(crtc.out_fence_ptr, 0);
            });
        }
        if let Err(e) = changes.commit(flags, 0) {
            log::debug!("Could not deactivate crtcs: {}", ErrorFmt(e));
            return false;
        }

        true
    }

    fn create_scanout_buffers(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &Format,
        plane_modifiers: &IndexSet<Modifier>,
        width: i32,
        height: i32,
        ctx: &MetalRenderContext,
        cursor: bool,
    ) -> Result<[RenderBuffer; 2], MetalError> {
        let create =
            || self.create_scanout_buffer(dev, format, plane_modifiers, width, height, ctx, cursor);
        Ok([create()?, create()?])
    }

    fn create_scanout_buffer(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &Format,
        plane_modifiers: &IndexSet<Modifier>,
        width: i32,
        height: i32,
        render_ctx: &MetalRenderContext,
        cursor: bool,
    ) -> Result<RenderBuffer, MetalError> {
        let ctx = dev.ctx.get();
        let dev_gfx_formats = ctx.gfx.formats();
        let dev_gfx_format = match dev_gfx_formats.get(&format.drm) {
            None => return Err(MetalError::MissingDevFormat(format.name)),
            Some(f) => f,
        };
        let possible_modifiers: Vec<_> = dev_gfx_format
            .write_modifiers
            .iter()
            .filter(|m| plane_modifiers.contains(*m))
            .copied()
            .collect();
        if possible_modifiers.is_empty() {
            log::warn!("Scanout modifiers: {:?}", plane_modifiers);
            log::warn!("DEV GFX modifiers: {:?}", dev_gfx_format.write_modifiers);
            return Err(MetalError::MissingDevModifier(format.name));
        }
        let mut usage = GBM_BO_USE_RENDERING | GBM_BO_USE_SCANOUT;
        if cursor {
            usage |= GBM_BO_USE_LINEAR;
        };
        let dev_bo = dev.gbm.create_bo(
            &self.state.dma_buf_ids,
            width,
            height,
            format,
            &possible_modifiers,
            usage,
        );
        let dev_bo = match dev_bo {
            Ok(b) => b,
            Err(e) => return Err(MetalError::ScanoutBuffer(e)),
        };
        let drm_fb = match dev.master.add_fb(dev_bo.dmabuf(), None) {
            Ok(fb) => Rc::new(fb),
            Err(e) => return Err(MetalError::Framebuffer(e)),
        };
        let dev_img = match ctx.gfx.clone().dmabuf_img(dev_bo.dmabuf()) {
            Ok(img) => img,
            Err(e) => return Err(MetalError::ImportImage(e)),
        };
        let dev_fb = match dev_img.clone().to_framebuffer() {
            Ok(fb) => fb,
            Err(e) => return Err(MetalError::ImportFb(e)),
        };
        dev_fb.clear();
        let (dev_tex, render_tex, render_fb) = if dev.id == render_ctx.dev_id {
            let render_tex = match dev_img.to_texture() {
                Ok(fb) => fb,
                Err(e) => return Err(MetalError::ImportTexture(e)),
            };
            (None, render_tex, None)
        } else {
            // Create a _bridge_ BO in the render device
            let render_gfx_formats = render_ctx.gfx.formats();
            let render_gfx_format = match render_gfx_formats.get(&format.drm) {
                None => return Err(MetalError::MissingRenderFormat(format.name)),
                Some(f) => f,
            };
            let possible_modifiers: Vec<_> = render_gfx_format
                .write_modifiers
                .iter()
                .filter(|m| dev_gfx_format.read_modifiers.contains(*m))
                .copied()
                .collect();
            if possible_modifiers.is_empty() {
                log::warn!(
                    "Render GFX modifiers: {:?}",
                    render_gfx_format.write_modifiers
                );
                log::warn!("DEV GFX modifiers: {:?}", dev_gfx_format.read_modifiers);
                return Err(MetalError::MissingRenderModifier(format.name));
            }
            usage = GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR;
            let render_bo = render_ctx.gfx.gbm().create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                &possible_modifiers,
                usage,
            );
            let render_bo = match render_bo {
                Ok(b) => b,
                Err(e) => return Err(MetalError::ScanoutBuffer(e)),
            };
            let render_img = match render_ctx.gfx.clone().dmabuf_img(render_bo.dmabuf()) {
                Ok(img) => img,
                Err(e) => return Err(MetalError::ImportImage(e)),
            };
            let render_fb = match render_img.clone().to_framebuffer() {
                Ok(fb) => fb,
                Err(e) => return Err(MetalError::ImportFb(e)),
            };
            render_fb.clear();
            let render_tex = match render_img.to_texture() {
                Ok(fb) => fb,
                Err(e) => return Err(MetalError::ImportTexture(e)),
            };

            // Import the bridge BO into the current device
            let dev_img = match ctx.gfx.clone().dmabuf_img(render_bo.dmabuf()) {
                Ok(img) => img,
                Err(e) => return Err(MetalError::ImportImage(e)),
            };
            let dev_tex = match dev_img.to_texture() {
                Ok(fb) => fb,
                Err(e) => return Err(MetalError::ImportTexture(e)),
            };

            (Some(dev_tex), render_tex, Some(render_fb))
        };
        Ok(RenderBuffer {
            drm: drm_fb,
            dev_fb,
            dev_tex,
            render_tex,
            render_fb,
        })
    }

    fn assign_connector_crtc(
        &self,
        connector: &Rc<MetalConnector>,
        changes: &mut Change,
    ) -> Result<(), MetalError> {
        let dd = connector.display.borrow_mut();
        if !connector.enabled.get() || dd.connection != ConnectorStatus::Connected {
            return Ok(());
        }
        let crtc = 'crtc: {
            for crtc in dd.crtcs.values() {
                if crtc.connector.get().is_none() {
                    break 'crtc crtc.clone();
                }
            }
            return Err(MetalError::NoCrtcForConnector);
        };
        let mode = match &dd.mode {
            Some(m) => m,
            _ => return Err(MetalError::NoModeForConnector),
        };
        let mode_blob = mode.create_blob(&connector.master)?;
        changes.change_object(connector.id, |c| {
            c.change(dd.crtc_id.id, crtc.id.0 as _);
        });
        changes.change_object(crtc.id, |c| {
            c.change(crtc.active.id, 1);
            c.change(crtc.mode_id.id, mode_blob.id().0 as _);
        });
        connector.crtc.set(Some(crtc.clone()));
        dd.crtc_id.value.set(crtc.id);
        crtc.connector.set(Some(connector.clone()));
        crtc.active.value.set(true);
        crtc.mode_id.value.set(mode_blob.id());
        crtc.mode_blob.set(Some(Rc::new(mode_blob)));
        Ok(())
    }

    fn assign_connector_planes(
        &self,
        connector: &Rc<MetalConnector>,
        changes: &mut Change,
        ctx: &MetalRenderContext,
        old_buffers: &mut Vec<Rc<[RenderBuffer; 2]>>,
    ) -> Result<(), MetalError> {
        let dd = connector.display.borrow_mut();
        let crtc = match connector.crtc.get() {
            Some(c) => c,
            _ => return Ok(()),
        };
        let mode = match &dd.mode {
            Some(m) => m,
            _ => {
                log::error!("Connector has a crtc assigned but no mode");
                return Ok(());
            }
        };
        let (primary_plane, primary_modifiers) = 'primary_plane: {
            for plane in crtc.possible_planes.values() {
                if plane.ty == PlaneType::Primary && !plane.assigned.get() {
                    if let Some(format) = plane.formats.get(&XRGB8888.drm) {
                        break 'primary_plane (plane.clone(), &format.modifiers);
                    }
                }
            }
            return Err(MetalError::NoPrimaryPlaneForConnector);
        };
        let buffers = Rc::new(self.create_scanout_buffers(
            &connector.dev,
            XRGB8888,
            primary_modifiers,
            mode.hdisplay as _,
            mode.vdisplay as _,
            ctx,
            false,
        )?);
        let mut cursor_plane = None;
        let mut cursor_modifiers = &IndexSet::new();
        for plane in crtc.possible_planes.values() {
            if plane.ty == PlaneType::Cursor
                && !plane.assigned.get()
                && plane.formats.contains_key(&ARGB8888.drm)
            {
                if let Some(format) = plane.formats.get(&ARGB8888.drm) {
                    cursor_plane = Some(plane.clone());
                    cursor_modifiers = &format.modifiers;
                    break;
                }
            }
        }
        let mut cursor_buffers = None;
        if cursor_plane.is_some() {
            let res = self.create_scanout_buffers(
                &connector.dev,
                ARGB8888,
                cursor_modifiers,
                connector.dev.cursor_width as _,
                connector.dev.cursor_height as _,
                ctx,
                true,
            );
            match res {
                Ok(r) => cursor_buffers = Some(Rc::new(r)),
                Err(e) => {
                    log::warn!(
                        "Could not allocate buffers for the cursor plane: {}",
                        ErrorFmt(e)
                    );
                    cursor_plane = None;
                }
            }
        }
        changes.change_object(primary_plane.id, |c| {
            c.change(primary_plane.fb_id, buffers[0].drm.id().0 as _);
            c.change(primary_plane.crtc_id.id, crtc.id.0 as _);
            c.change(primary_plane.crtc_x.id, 0);
            c.change(primary_plane.crtc_y.id, 0);
            c.change(primary_plane.crtc_w.id, mode.hdisplay as _);
            c.change(primary_plane.crtc_h.id, mode.vdisplay as _);
            c.change(primary_plane.src_x.id, 0);
            c.change(primary_plane.src_y.id, 0);
            c.change(primary_plane.src_w.id, (mode.hdisplay as u64) << 16);
            c.change(primary_plane.src_h.id, (mode.vdisplay as u64) << 16);
        });
        primary_plane.assigned.set(true);
        primary_plane.mode_w.set(mode.hdisplay as _);
        primary_plane.mode_h.set(mode.vdisplay as _);
        primary_plane.crtc_id.value.set(crtc.id);
        primary_plane.crtc_x.value.set(0);
        primary_plane.crtc_y.value.set(0);
        primary_plane.crtc_w.value.set(mode.hdisplay as _);
        primary_plane.crtc_h.value.set(mode.vdisplay as _);
        primary_plane.src_x.value.set(0);
        primary_plane.src_y.value.set(0);
        primary_plane.src_w.value.set((mode.hdisplay as u32) << 16);
        primary_plane.src_h.value.set((mode.vdisplay as u32) << 16);
        if let Some(old) = connector.buffers.set(Some(buffers)) {
            old_buffers.push(old);
        }
        connector.primary_plane.set(Some(primary_plane.clone()));
        if let Some(cp) = &cursor_plane {
            cp.assigned.set(true);
        }
        if let Some(old) = connector.cursor_buffers.set(cursor_buffers) {
            old_buffers.push(old);
        }
        connector.cursor_plane.set(cursor_plane);
        connector.cursor_enabled.set(false);
        Ok(())
    }

    fn start_connector(&self, connector: &Rc<MetalConnector>, log_mode: bool) {
        let dd = connector.display.borrow_mut();
        if !connector.connect_sent.get() {
            self.send_connected(connector, &dd);
        }
        if log_mode {
            log::info!(
                "Initialized connector {}-{} with mode {:?}",
                dd.connector_type,
                dd.connector_type_id,
                dd.mode.as_ref().unwrap(),
            );
        }
        connector.has_damage.set(true);
        connector.cursor_changed.set(true);
        connector.schedule_present();
    }
}

#[derive(Debug)]
pub struct RenderBuffer {
    drm: Rc<DrmFramebuffer>,
    // ctx = dev
    // buffer location = dev
    dev_fb: Rc<dyn GfxFramebuffer>,
    // ctx = dev
    // buffer location = render
    dev_tex: Option<Rc<dyn GfxTexture>>,
    // ctx = render
    // buffer location = render
    render_tex: Rc<dyn GfxTexture>,
    // ctx = render
    // buffer location = render
    render_fb: Option<Rc<dyn GfxFramebuffer>>,
}

impl RenderBuffer {
    fn render_fb(&self) -> Rc<dyn GfxFramebuffer> {
        self.render_fb
            .clone()
            .unwrap_or_else(|| self.dev_fb.clone())
    }
}

fn modes_equal(a: &DrmModeInfo, b: &DrmModeInfo) -> bool {
    a.clock == b.clock
        && a.hdisplay == b.hdisplay
        && a.hsync_start == b.hsync_start
        && a.hsync_end == b.hsync_end
        && a.htotal == b.htotal
        && a.hskew == b.hskew
        && a.vdisplay == b.vdisplay
        && a.vsync_start == b.vsync_start
        && a.vsync_end == b.vsync_end
        && a.vtotal == b.vtotal
        && a.vscan == b.vscan
        && a.vrefresh == b.vrefresh
        && a.flags == b.flags
}

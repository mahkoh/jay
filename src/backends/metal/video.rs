use {
    crate::{
        allocator::BufferObject,
        async_engine::{Phase, SpawnedFuture},
        backend::{
            BackendColorSpace, BackendConnectorState, BackendDrmDevice, BackendDrmLease,
            BackendDrmLessee, BackendEotfs, BackendEvent, BackendLuminance, CONCAP_CONNECTOR,
            CONCAP_MODE_SETTING, CONCAP_PHYSICAL_DISPLAY, Connector, ConnectorCaps, ConnectorEvent,
            ConnectorId, ConnectorKernelId, DrmDeviceId, HardwareCursor, HardwareCursorUpdate,
            Mode, MonitorInfo,
            transaction::{
                BackendConnectorTransaction, BackendConnectorTransactionError,
                BackendConnectorTransactionType, BackendConnectorTransactionTypeDyn,
            },
        },
        backends::metal::{
            MetalBackend, MetalError, ScanoutBufferError, ScanoutBufferErrorKind,
            present::{
                DEFAULT_POST_COMMIT_MARGIN, DEFAULT_PRE_COMMIT_MARGIN, DirectScanoutCache,
                POST_COMMIT_MARGIN_DELTA, PresentFb,
            },
            transaction::{DrmConnectorState, DrmCrtcState, DrmPlaneState, MetalDeviceTransaction},
        },
        cmm::{cmm_description::ColorDescription, cmm_primaries::Primaries},
        drm_feedback::DrmFeedback,
        edid::{CtaDataBlock, Descriptor, EdidExtension},
        format::{Format, XRGB8888},
        gfx_api::{
            AcquireSync, GfxBlendBuffer, GfxContext, GfxFramebuffer, GfxTexture, ReleaseSync,
            SyncFile, needs_render_usage,
        },
        ifs::{
            wl_output::OutputId,
            wp_presentation_feedback::{KIND_HW_COMPLETION, KIND_VSYNC, KIND_ZERO_COPY},
        },
        rect::{DamageQueue, Rect},
        state::State,
        tree::OutputNode,
        udev::UdevDevice,
        utils::{
            asyncevent::AsyncEvent, binary_search_map::BinarySearchMap, bitflags::BitflagsExt,
            cell_ext::CellExt, clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt,
            geometric_decay::GeometricDecay, numcell::NumCell, on_change::OnChange,
            opaque_cell::OpaqueCell, ordered_float::F64, oserror::OsError,
        },
        video::{
            INVALID_MODIFIER, Modifier,
            dmabuf::DmaBufId,
            drm::{
                ConnectorStatus, ConnectorType, DRM_CLIENT_CAP_ATOMIC, DrmBlob, DrmConnector,
                DrmCrtc, DrmEncoder, DrmError, DrmEvent, DrmFb, DrmFramebuffer, DrmLease,
                DrmMaster, DrmModeInfo, DrmObject, DrmPlane, DrmProperty, DrmPropertyDefinition,
                DrmPropertyType, DrmVersion, HDMI_EOTF_TRADITIONAL_GAMMA_SDR, drm_mode_modeinfo,
                hdr_output_metadata,
            },
            gbm::{GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING, GBM_BO_USE_SCANOUT, GbmBo, GbmDevice},
        },
    },
    ahash::{AHashMap, AHashSet},
    arrayvec::ArrayVec,
    bstr::{BString, ByteSlice},
    indexmap::{IndexMap, IndexSet, indexset},
    isnt::std_1::collections::IsntHashMapExt,
    jay_config::video::GfxApi,
    run_on_drop::on_drop,
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        ffi::CString,
        fmt::{Debug, Formatter},
        mem,
        ops::DerefMut,
        rc::Rc,
    },
    uapi::{
        OwnedFd,
        c::{self, dev_t},
    },
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
    pub gbm: Rc<GbmDevice>,
    pub devnode: CString,
}

pub struct MetalDrmDevice {
    pub backend: Rc<MetalBackend>,
    pub id: DrmDeviceId,
    pub devnum: c::dev_t,
    pub devnode: CString,
    pub master: Rc<DrmMaster>,
    pub crtcs: AHashMap<DrmCrtc, Rc<MetalCrtc>>,
    pub encoders: AHashMap<DrmEncoder, Rc<MetalEncoder>>,
    pub planes: AHashMap<DrmPlane, Rc<MetalPlane>>,
    pub _min_width: u32,
    pub _max_width: u32,
    pub _min_height: u32,
    pub _max_height: u32,
    pub cursor_width: u64,
    pub cursor_height: u64,
    pub supports_async_commit: bool,
    pub gbm: Rc<GbmDevice>,
    pub handle_events: HandleEvents,
    pub ctx: CloneCell<Rc<MetalRenderContext>>,
    pub on_change: OnChange<crate::backend::DrmEvent>,
    pub direct_scanout_enabled: Cell<Option<bool>>,
    pub is_nvidia: bool,
    pub _is_amd: bool,
    pub lease_ids: MetalLeaseIds,
    pub leases: CopyHashMap<MetalLeaseId, MetalLeaseData>,
    pub leases_to_break: CopyHashMap<MetalLeaseId, MetalLeaseData>,
    pub paused: Cell<bool>,
    pub min_post_commit_margin: Cell<u64>,
}

impl Debug for MetalDrmDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalDrmDevice").finish_non_exhaustive()
    }
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

    fn is_render_device(&self) -> bool {
        Some(self.id) == self.backend.ctx.get().map(|c| c.dev_id)
    }

    fn create_lease(
        self: Rc<Self>,
        lessee: Rc<dyn BackendDrmLessee>,
        connector_ids: &[ConnectorId],
    ) {
        let Some(data) = self.backend.device_holder.drm_devices.get(&self.devnum) else {
            log::error!("Tried to create a lease for a DRM device that no longer exists");
            return;
        };
        let mut connectors = vec![];
        let mut crtcs = AHashMap::new();
        let mut planes = AHashMap::new();
        let mut ids = vec![];
        for id in connector_ids {
            let Some(connector) = data
                .connectors
                .lock()
                .values()
                .find(|c| c.connector_id == *id)
                .cloned()
            else {
                log::error!("Tried to lease connector {id} but no such connector exists");
                return;
            };
            let fe_state = connector.frontend_state.get();
            match fe_state {
                FrontState::Connected { non_desktop: true } => {}
                FrontState::Connected { non_desktop: false }
                | FrontState::Removed
                | FrontState::Disconnected
                | FrontState::Unavailable => {
                    log::error!(
                        "Tried to lease connector {id} but it is in an invalid state: {fe_state:?}"
                    );
                    return;
                }
            }
            if let Some(lease_id) = connector.lease.get() {
                match data.dev.leases_to_break.lock().entry(lease_id) {
                    Entry::Occupied(oe) => {
                        if oe.get().try_revoke() {
                            oe.remove();
                        }
                    }
                    _ => {
                        log::error!(
                            "Connector is logically available for leasing, has a lease ID, and has no entry in leases_to_break"
                        );
                    }
                }
            }
            if connector.lease.is_some() {
                log::error!("Tried to lease connector {id} but it is already leased");
                return;
            }
            let dd = &*connector.display.borrow();
            let crtc = dd.crtcs.values().find(|c| {
                c.connector.is_none() && c.lease.is_none() && crtcs.not_contains_key(&c.id)
            });
            let Some(crtc) = crtc else {
                log::error!("Tried to lease connector {id} but it has no matching unused CRTC");
                return;
            };
            let plane = crtc.possible_planes.values().find(|p| {
                p.drm_state.borrow().assigned_crtc.is_none()
                    && p.lease.is_none()
                    && planes.not_contains_key(&p.id)
                    && p.ty == PlaneType::Primary
            });
            let Some(plane) = plane else {
                log::error!("Tried to lease connector {id} but it has no matching unused plane");
                return;
            };
            connectors.push(connector.clone());
            crtcs.insert(crtc.id, crtc.clone());
            planes.insert(plane.id, plane.clone());
            ids.push(connector.id.0);
            ids.push(crtc.id.0);
            ids.push(plane.id.0);
        }
        let drm_lease = match self.master.lease(&ids) {
            Ok(l) => l,
            Err(e) => {
                log::error!("Could not create lease: {}", ErrorFmt(e));
                return;
            }
        };
        let lease_id = self.lease_ids.next();
        for c in &connectors {
            c.lease.set(Some(lease_id));
            c.send_event(ConnectorEvent::Unavailable);
        }
        for c in crtcs.values() {
            c.lease.set(Some(lease_id));
        }
        for p in planes.values() {
            p.lease.set(Some(lease_id));
        }
        let fd = drm_lease.lessee_fd().clone();
        let lease_data = MetalLeaseData {
            lease: drm_lease,
            _lessee: lessee.clone(),
            connectors,
            crtcs: crtcs.values().cloned().collect(),
            planes: planes.values().cloned().collect(),
            revoked: Cell::new(false),
        };
        self.leases.set(lease_id, lease_data);
        let lease = Rc::new(MetalLease {
            dev: self.clone(),
            id: lease_id,
            fd,
        });
        lessee.created(lease);
    }

    fn set_flip_margin(&self, margin: u64) {
        self.min_post_commit_margin.set(margin);
        if let Some(dd) = self.backend.device_holder.drm_devices.get(&self.devnum) {
            for c in dd.connectors.lock().values() {
                c.post_commit_margin.set(margin);
                c.post_commit_margin_decay.reset(margin);
                if let Some(output) = self.backend.state.root.outputs.get(&c.connector_id) {
                    output.flip_margin_ns.set(Some(margin));
                }
            }
        }
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
}

#[derive(Debug)]
pub struct PersistentDisplayData {
    pub state: RefCell<BackendConnectorState>,
}

#[derive(Debug)]
pub struct DefaultProperty {
    pub name: &'static str,
    pub prop: DrmProperty,
    pub value: u64,
}

#[derive(Debug)]
pub struct ConnectorDisplayData {
    pub crtc_id: DrmProperty,
    pub crtcs: BinarySearchMap<DrmCrtc, Rc<MetalCrtc>, 8>,
    pub first_mode: Mode,
    pub modes: Vec<DrmModeInfo>,
    pub persistent: Rc<PersistentDisplayData>,
    pub refresh: u32,
    pub non_desktop: bool,
    pub non_desktop_effective: bool,
    pub vrr_capable: bool,
    pub _vrr_refresh_max_nsec: u64,
    pub default_properties: Vec<DefaultProperty>,
    pub untyped_properties: AHashMap<DrmProperty, u64>,

    pub connector_id: ConnectorKernelId,
    pub output_id: Rc<OutputId>,

    pub connection: ConnectorStatus,
    pub mm_width: u32,
    pub mm_height: u32,
    pub _subpixel: u32,

    pub supports_bt2020: bool,
    pub supports_pq: bool,
    pub primaries: Primaries,
    pub luminance: Option<BackendLuminance>,

    pub colorspace: Option<DrmProperty>,
    pub hdr_metadata: Option<DrmProperty>,
    pub drm_state: DrmConnectorState,
}

impl ConnectorDisplayData {
    fn update_refresh(&mut self, dev: &MetalDrmDevice) {
        self.refresh = 0;
        if self.drm_state.crtc_id.is_none() {
            return;
        }
        let Some(crtc) = dev.crtcs.get(&self.drm_state.crtc_id) else {
            return;
        };
        let drm_state = &*crtc.drm_state.borrow();
        let Some(mode) = &drm_state.mode else {
            return;
        };
        let refresh_rate_mhz = mode.refresh_rate_millihz();
        if refresh_rate_mhz != 0 {
            self.refresh = (1_000_000_000_000u64 / refresh_rate_mhz as u64) as u32;
        }
    }

    fn update_non_desktop_effective(&mut self) {
        let state = &*self.persistent.state.borrow();
        self.non_desktop_effective =
            !state.enabled || state.non_desktop_override.unwrap_or(self.non_desktop);
    }

    pub fn update_cached_fields(&mut self, dev: &MetalDrmDevice) {
        self.update_refresh(dev);
        self.update_non_desktop_effective();
    }
}

linear_ids!(MetalLeaseIds, MetalLeaseId, u64);

pub struct MetalLeaseData {
    pub lease: DrmLease,
    pub _lessee: Rc<dyn BackendDrmLessee>,
    pub connectors: Vec<Rc<MetalConnector>>,
    pub crtcs: Vec<Rc<MetalCrtc>>,
    pub planes: Vec<Rc<MetalPlane>>,
    pub revoked: Cell<bool>,
}

impl MetalLeaseData {
    fn try_revoke(&self) -> bool {
        if self.revoked.get() {
            return true;
        }
        let res = self.lease.try_revoke();
        if res {
            self.revoked.set(res);
            for c in &self.connectors {
                c.lease.take();
                if let Err(e) = c.update_properties() {
                    log::error!("Could not update connector properties: {}", ErrorFmt(e));
                }
            }
            for c in &self.crtcs {
                c.lease.take();
                if let Err(e) = c.update_properties() {
                    log::error!("Could not update crtc properties: {}", ErrorFmt(e));
                }
            }
            for p in &self.planes {
                p.lease.take();
                if let Err(e) = p.update_properties() {
                    log::error!("Could not update plane properties: {}", ErrorFmt(e));
                }
            }
        }
        res
    }
}

pub struct MetalLease {
    dev: Rc<MetalDrmDevice>,
    id: MetalLeaseId,
    fd: Rc<OwnedFd>,
}

impl Drop for MetalLease {
    fn drop(&mut self) {
        if let Some(lease) = self.dev.leases.remove(&self.id) {
            if !self.dev.paused.get() {
                for c in &lease.connectors {
                    match c.frontend_state.get() {
                        FrontState::Removed
                        | FrontState::Disconnected
                        | FrontState::Connected { .. } => {}
                        FrontState::Unavailable => {
                            c.send_event(ConnectorEvent::Available);
                        }
                    }
                }
            }
            if !lease.try_revoke() {
                self.dev.leases_to_break.set(self.id, lease);
            }
        }
    }
}

impl BackendDrmLease for MetalLease {
    fn fd(&self) -> &Rc<OwnedFd> {
        &self.fd
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FrontState {
    Removed,
    Disconnected,
    Connected { non_desktop: bool },
    Unavailable,
}

pub struct MetalConnector {
    pub id: DrmConnector,
    pub kernel_id: Cell<ConnectorKernelId>,
    pub master: Rc<DrmMaster>,
    pub state: Rc<State>,

    pub dev: Rc<MetalDrmDevice>,
    pub backend: Rc<MetalBackend>,

    pub connector_id: ConnectorId,

    pub buffers: CloneCell<Option<Rc<[RenderBuffer; 2]>>>,
    pub color_description: CloneCell<Rc<ColorDescription>>,

    pub lease: Cell<Option<MetalLeaseId>>,

    pub buffers_idle: Cell<bool>,
    pub crtc_idle: Cell<bool>,
    pub has_damage: NumCell<u64>,
    pub cursor_changed: Cell<bool>,
    pub cursor_damage: Cell<bool>,
    pub next_vblank_nsec: Cell<u64>,

    pub display: RefCell<ConnectorDisplayData>,

    pub frontend_state: Cell<FrontState>,

    pub primary_plane: CloneCell<Option<Rc<MetalPlane>>>,
    pub cursor_plane: CloneCell<Option<Rc<MetalPlane>>>,

    pub crtc: CloneCell<Option<Rc<MetalCrtc>>>,

    pub on_change: OnChange<ConnectorEvent>,

    pub present_trigger: AsyncEvent,

    pub cursor_x: Cell<i32>,
    pub cursor_y: Cell<i32>,
    pub cursor_enabled: Cell<bool>,
    pub cursor_buffers: CloneCell<Option<Rc<[RenderBuffer; 2]>>>,
    pub cursor_swap_buffer: Cell<bool>,
    pub cursor_sync_file: CloneCell<Option<SyncFile>>,

    pub drm_feedback: CloneCell<Option<Rc<DrmFeedback>>>,
    pub scanout_buffers: RefCell<AHashMap<DmaBufId, DirectScanoutCache>>,
    pub active_framebuffer: RefCell<Option<PresentFb>>,
    pub next_framebuffer: OpaqueCell<Option<PresentFb>>,
    pub direct_scanout_active: Cell<bool>,

    pub version: NumCell<u64>,
    pub expected_sequence: Cell<Option<u64>>,
    pub pre_commit_margin: Cell<u64>,
    pub pre_commit_margin_decay: GeometricDecay,
    pub post_commit_margin: Cell<u64>,
    pub post_commit_margin_decay: GeometricDecay,
    pub vblank_miss_sec: Cell<u32>,
    pub vblank_miss_this_sec: NumCell<u32>,
    pub presentation_is_sync: Cell<bool>,
    pub presentation_is_zero_copy: Cell<bool>,
}

impl Debug for MetalConnector {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalConnnector").finish_non_exhaustive()
    }
}

pub struct MetalHardwareCursor {
    pub connector: Rc<MetalConnector>,
}

pub struct MetalHardwareCursorChange<'a> {
    pub cursor_swap_buffer: bool,
    pub cursor_enabled: bool,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub cursor_buffer: &'a RenderBuffer,
    pub sync_file: Option<SyncFile>,
    pub cursor_size: (i32, i32),
}

impl Debug for MetalHardwareCursor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalHardwareCursor")
            .finish_non_exhaustive()
    }
}

impl HardwareCursor for MetalHardwareCursor {
    fn damage(&self) {
        self.connector.cursor_damage.set(true);
        if self.connector.buffers_idle.get() && self.connector.crtc_idle.get() {
            self.connector.schedule_present();
        }
    }
}

impl HardwareCursorUpdate for MetalHardwareCursorChange<'_> {
    fn set_enabled(&mut self, enabled: bool) {
        self.cursor_enabled = enabled;
    }

    fn get_buffer(&self) -> Rc<dyn GfxFramebuffer> {
        self.cursor_buffer.render_fb()
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    fn swap_buffer(&mut self) {
        self.cursor_swap_buffer = true;
    }

    fn set_sync_file(&mut self, sync_file: Option<SyncFile>) {
        self.sync_file = sync_file;
    }

    fn size(&self) -> (i32, i32) {
        self.cursor_size
    }
}

pub struct ConnectorFutures {
    pub _present: SpawnedFuture<()>,
}

impl Debug for ConnectorFutures {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectorFutures").finish_non_exhaustive()
    }
}

impl MetalConnector {
    pub fn send_connected(self: &Rc<Self>) {
        let dd = &*self.display.borrow();
        self.backend.send_connected(self, dd);
    }

    pub fn send_state(&self) {
        match self.frontend_state.get() {
            FrontState::Removed
            | FrontState::Disconnected
            | FrontState::Unavailable
            | FrontState::Connected { non_desktop: true } => return,
            FrontState::Connected { non_desktop: false } => {}
        }
        let mut state = *self.display.borrow().persistent.state.borrow();
        state.serial = self.state.backend_connector_state_serials.next();
        self.send_event(ConnectorEvent::State(state));
    }

    pub fn send_formats(&self) {
        match self.frontend_state.get() {
            FrontState::Removed
            | FrontState::Disconnected
            | FrontState::Unavailable
            | FrontState::Connected { non_desktop: true } => return,
            FrontState::Connected { non_desktop: false } => {}
        }
        let mut formats = vec![];
        if let Some(plane) = self.primary_plane.get() {
            formats = plane.formats.values().map(|f| f.format).collect();
        }
        let formats = Rc::new(formats);
        self.send_event(ConnectorEvent::FormatsChanged(formats));
    }

    pub fn send_hardware_cursor(self: &Rc<Self>) {
        match self.frontend_state.get() {
            FrontState::Removed
            | FrontState::Disconnected
            | FrontState::Unavailable
            | FrontState::Connected { non_desktop: true } => return,
            FrontState::Connected { non_desktop: false } => {}
        }
        let hc = self.cursor_buffers.is_some().then(|| {
            Rc::new(MetalHardwareCursor {
                connector: self.clone(),
            }) as _
        });
        self.on_change
            .send_event(ConnectorEvent::HardwareCursor(hc));
    }

    fn connected(&self) -> bool {
        let dd = self.display.borrow();
        dd.connection == ConnectorStatus::Connected
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

    pub fn send_event(&self, event: ConnectorEvent) {
        let state = self.frontend_state.get();
        macro_rules! desktop_event {
            ($name:expr) => {
                match state {
                    FrontState::Connected { non_desktop: false } => {
                        self.on_change.send_event(event);
                    }
                    FrontState::Connected { non_desktop: true }
                    | FrontState::Removed
                    | FrontState::Disconnected
                    | FrontState::Unavailable => {
                        log::error!("Tried to send {} event in invalid state: {state:?}", $name);
                    }
                }
            };
        }
        let set_state = |ns: FrontState| {
            log::debug!(
                "Changing state of {}: {state:?} -> {ns:?}",
                self.kernel_id.get(),
            );
            self.frontend_state.set(ns);
        };
        match &event {
            ConnectorEvent::Connected(ty) => match state {
                FrontState::Disconnected => {
                    let non_desktop = ty.non_desktop_effective;
                    self.on_change.send_event(event);
                    set_state(FrontState::Connected { non_desktop });
                }
                FrontState::Removed | FrontState::Connected { .. } | FrontState::Unavailable => {
                    log::error!("Tried to send connected event in invalid state: {state:?}");
                }
            },
            ConnectorEvent::HardwareCursor(_) => {
                desktop_event!("hardware cursor");
            }
            ConnectorEvent::State(_) => {
                desktop_event!("state");
            }
            ConnectorEvent::Disconnected => match state {
                FrontState::Connected { .. } | FrontState::Unavailable => {
                    self.on_change.send_event(event);
                    set_state(FrontState::Disconnected);
                }
                FrontState::Removed | FrontState::Disconnected => {
                    log::error!("Tried to send disconnected event in invalid state: {state:?}");
                }
            },
            ConnectorEvent::Removed => match state {
                FrontState::Disconnected => {
                    self.on_change.send_event(event);
                    set_state(FrontState::Removed);
                }
                FrontState::Removed | FrontState::Connected { .. } | FrontState::Unavailable => {
                    log::error!("Tried to send removed event in invalid state: {state:?}");
                }
            },
            ConnectorEvent::Unavailable => match state {
                FrontState::Connected { non_desktop: true } => {
                    self.on_change.send_event(event);
                    set_state(FrontState::Unavailable);
                }
                FrontState::Connected { non_desktop: false }
                | FrontState::Removed
                | FrontState::Disconnected
                | FrontState::Unavailable => {
                    log::error!("Tried to send unavailable event in invalid state: {state:?}");
                }
            },
            ConnectorEvent::Available => match state {
                FrontState::Unavailable => {
                    self.on_change.send_event(event);
                    set_state(FrontState::Connected { non_desktop: true });
                }
                FrontState::Connected { .. } | FrontState::Removed | FrontState::Disconnected => {
                    log::error!("Tried to send available event in invalid state: {state:?}");
                }
            },
            ConnectorEvent::FormatsChanged(_) => {
                desktop_event!("formats-changed");
            }
        }
    }

    fn queue_sequence(&self) {
        if let Some(crtc) = self.crtc.get() {
            crtc.queue_sequence();
        }
    }
}

impl MetalCrtc {
    fn queue_sequence(&self) {
        if self.needs_vblank_emulation.get() {
            return;
        }
        if let Err(e) = self.master.queue_sequence(self.id) {
            log::error!("Could not queue a CRTC sequence: {}", ErrorFmt(&e));
            if let DrmError::QueueSequence(OsError(c::EOPNOTSUPP)) = e
                && let Some(connector) = self.connector.get()
                && let Some(node) = connector.state.root.outputs.get(&connector.connector_id)
            {
                log::warn!("{}: Switching to vblank emulation", connector.kernel_id());
                self.needs_vblank_emulation.set(true);
                node.global.connector.needs_vblank_emulation.set(true);
                node.vblank();
            }
        } else {
            self.have_queued_sequence.set(true);
        }
    }
}

impl Connector for MetalConnector {
    fn id(&self) -> ConnectorId {
        self.connector_id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        self.kernel_id.get()
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.on_change.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.on_change.set(Some(cb));
    }

    fn damage(&self) {
        self.has_damage.fetch_add(1);
        if self.buffers_idle.get() && self.crtc_idle.get() {
            self.schedule_present();
        }
    }

    fn drm_dev(&self) -> Option<DrmDeviceId> {
        Some(self.dev.id)
    }

    fn effectively_locked(&self) -> bool {
        let dd = &*self.display.borrow();
        let state = &*dd.persistent.state.borrow();
        if !state.enabled || !state.active {
            return true;
        }
        let Some(fb) = &*self.active_framebuffer.borrow() else {
            return false;
        };
        fb.locked
    }

    fn caps(&self) -> ConnectorCaps {
        CONCAP_CONNECTOR | CONCAP_MODE_SETTING | CONCAP_PHYSICAL_DISPLAY
    }

    fn drm_feedback(&self) -> Option<Rc<DrmFeedback>> {
        self.drm_feedback.get()
    }

    fn drm_object_id(&self) -> Option<DrmConnector> {
        Some(self.id)
    }

    fn before_non_desktop_override_update(&self, overrd: Option<bool>) {
        {
            let dd = &*self.display.borrow();
            let old = dd.non_desktop_effective;
            let new = overrd.unwrap_or(dd.non_desktop);
            if old == new || new {
                return;
            }
        }
        if let Some(lease_id) = self.lease.get()
            && let Some(lease) = self.dev.leases.remove(&lease_id)
        {
            if lease.try_revoke() {
                self.send_event(ConnectorEvent::Available);
            } else {
                self.dev.leases_to_break.set(lease_id, lease);
            }
        }
    }

    fn transaction_type(&self) -> Box<dyn BackendConnectorTransactionTypeDyn> {
        #[derive(Eq, PartialEq, Hash)]
        struct TT(dev_t);
        impl BackendConnectorTransactionType for TT {}
        Box::new(TT(self.dev.devnum))
    }

    fn create_transaction(
        &self,
    ) -> Result<Box<dyn BackendConnectorTransaction>, BackendConnectorTransactionError> {
        self.create_transaction().map(|v| Box::new(v) as _)
    }
}

pub struct MetalCrtc {
    pub id: DrmCrtc,
    pub idx: usize,
    pub master: Rc<DrmMaster>,
    pub default_properties: Vec<DefaultProperty>,
    pub untyped_properties: RefCell<AHashMap<DrmProperty, u64>>,

    pub lease: Cell<Option<MetalLeaseId>>,

    pub possible_planes: BinarySearchMap<DrmPlane, Rc<MetalPlane>, 8>,

    pub connector: CloneCell<Option<Rc<MetalConnector>>>,
    pub pending_flip: CloneCell<Option<Rc<MetalConnector>>>,

    pub active: DrmProperty,
    pub mode_id: DrmProperty,
    pub vrr_enabled: DrmProperty,
    pub out_fence_ptr: DrmProperty,
    pub drm_state: RefCell<DrmCrtcState>,

    pub sequence: Cell<u64>,
    pub have_queued_sequence: Cell<bool>,
    pub needs_vblank_emulation: Cell<bool>,
}

impl Debug for MetalCrtc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalCrtc").finish_non_exhaustive()
    }
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
    pub format: &'static Format,
    pub modifiers: IndexSet<Modifier>,
}

pub struct MetalPlane {
    pub id: DrmPlane,
    pub master: Rc<DrmMaster>,
    pub default_properties: Vec<DefaultProperty>,
    pub untyped_properties: RefCell<AHashMap<DrmProperty, u64>>,

    pub ty: PlaneType,

    pub possible_crtcs: u32,
    pub formats: AHashMap<u32, PlaneFormat>,

    pub lease: Cell<Option<MetalLeaseId>>,

    pub mode_w: Cell<i32>,
    pub mode_h: Cell<i32>,

    pub crtc_id: DrmProperty,
    pub crtc_x: DrmProperty,
    pub crtc_y: DrmProperty,
    pub crtc_w: DrmProperty,
    pub crtc_h: DrmProperty,
    pub src_x: DrmProperty,
    pub src_y: DrmProperty,
    pub src_w: DrmProperty,
    pub src_h: DrmProperty,
    pub in_fence_fd: DrmProperty,
    pub fb_id: DrmProperty,

    pub drm_state: RefCell<DrmPlaneState>,
}

impl Debug for MetalPlane {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetalPlane").finish_non_exhaustive()
    }
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

#[derive(Copy, Clone)]
enum DefaultValue {
    Fixed(u64),
    Enum(&'static str),
    Bitmask(&'static [&'static str]),
    RangeMax,
}

fn create_default_properties(
    props: &CollectedProperties,
    defaults: &[(&'static str, DefaultValue)],
) -> Vec<DefaultProperty> {
    let mut res = vec![];
    let mut defaults = defaults.iter();
    'outer: loop {
        let Some(&(name, def)) = defaults.next() else {
            break;
        };
        if let Some((definition, _)) = props.props.get(name.as_bytes().as_bstr()) {
            let value = match def {
                DefaultValue::Fixed(v) => v,
                DefaultValue::Enum(e) => match &definition.ty {
                    DrmPropertyType::Enum {
                        values,
                        bitmask: false,
                    } => match values.iter().find(|v| v.name == e) {
                        None => continue,
                        Some(v) => v.value,
                    },
                    _ => continue,
                },
                DefaultValue::Bitmask(e) => match &definition.ty {
                    DrmPropertyType::Enum {
                        values,
                        bitmask: true,
                    } => {
                        let mut res = 0;
                        for &e in e {
                            match values.iter().find(|v| v.name == e) {
                                None => continue 'outer,
                                Some(v) => res |= 1 << v.value,
                            }
                        }
                        res
                    }
                    _ => continue,
                },
                DefaultValue::RangeMax => match &definition.ty {
                    DrmPropertyType::Range { max, .. } => *max,
                    DrmPropertyType::SignedRange { max, .. } => *max as u64,
                    _ => continue,
                },
            };
            res.push(DefaultProperty {
                name,
                prop: definition.id,
                value,
            });
        }
    }
    res
}

fn create_connector(
    backend: &Rc<MetalBackend>,
    connector: DrmConnector,
    dev: &Rc<MetalDrmDevice>,
) -> Result<(Rc<MetalConnector>, ConnectorFutures), DrmError> {
    let display = create_connector_display_data(connector, dev)?;
    log::info!(
        "Creating connector {} for device {}",
        display.connector_id,
        dev.devnode.as_bytes().as_bstr(),
    );
    let slf = Rc::new(MetalConnector {
        id: connector,
        kernel_id: Cell::new(display.connector_id),
        master: dev.master.clone(),
        state: backend.state.clone(),
        dev: dev.clone(),
        backend: backend.clone(),
        connector_id: backend.state.connector_ids.next(),
        buffers: Default::default(),
        color_description: CloneCell::new(backend.state.color_manager.srgb_gamma22().clone()),
        lease: Cell::new(None),
        buffers_idle: Cell::new(true),
        crtc_idle: Cell::new(true),
        has_damage: NumCell::new(1),
        primary_plane: Default::default(),
        cursor_plane: Default::default(),
        crtc: Default::default(),
        on_change: Default::default(),
        present_trigger: Default::default(),
        cursor_x: Cell::new(0),
        cursor_y: Cell::new(0),
        cursor_enabled: Cell::new(false),
        cursor_buffers: Default::default(),
        display: RefCell::new(display),
        frontend_state: Cell::new(FrontState::Removed),
        cursor_changed: Cell::new(false),
        cursor_damage: Cell::new(false),
        cursor_swap_buffer: Cell::new(false),
        cursor_sync_file: Default::default(),
        drm_feedback: Default::default(),
        scanout_buffers: Default::default(),
        active_framebuffer: Default::default(),
        next_framebuffer: Default::default(),
        direct_scanout_active: Cell::new(false),
        next_vblank_nsec: Cell::new(0),
        version: Default::default(),
        expected_sequence: Default::default(),
        pre_commit_margin_decay: GeometricDecay::new(0.5, DEFAULT_PRE_COMMIT_MARGIN),
        pre_commit_margin: Cell::new(DEFAULT_PRE_COMMIT_MARGIN),
        post_commit_margin_decay: GeometricDecay::new(0.1, dev.min_post_commit_margin.get()),
        post_commit_margin: Cell::new(dev.min_post_commit_margin.get()),
        vblank_miss_sec: Cell::new(0),
        vblank_miss_this_sec: Default::default(),
        presentation_is_sync: Cell::new(false),
        presentation_is_zero_copy: Cell::new(false),
    });
    let futures = ConnectorFutures {
        _present: backend.state.eng.spawn2(
            "present loop",
            Phase::Present,
            slf.clone().present_loop(),
        ),
    };
    Ok((slf, futures))
}

fn create_connector_display_data(
    connector: DrmConnector,
    dev: &Rc<MetalDrmDevice>,
) -> Result<ConnectorDisplayData, DrmError> {
    let info = dev.master.get_connector_info(connector, true)?;
    let mut crtcs = BinarySearchMap::new();
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
    let mut vrr_refresh_max_nsec = u64::MAX;
    let connector_id = ConnectorKernelId {
        ty: ConnectorType::from_drm(info.connector_type),
        idx: info.connector_type_id,
    };
    let mut supports_bt2020 = false;
    let mut supports_pq = false;
    let mut luminance = None;
    let mut primaries = Primaries::SRGB;
    'fetch_edid: {
        if connection != ConnectorStatus::Connected {
            break 'fetch_edid;
        }
        let edid = match props.get("EDID") {
            Ok(e) => e,
            _ => {
                log::warn!(
                    "Connector {} is connected but has no EDID blob",
                    connector_id,
                );
                break 'fetch_edid;
            }
        };
        let blob = match dev.master.getblob_vec::<u8>(DrmBlob(edid.value as _)) {
            Ok(b) => b,
            Err(e) => {
                log::error!(
                    "Could not fetch edid property of connector {}: {}",
                    connector_id,
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
                    connector_id,
                    ErrorFmt(e)
                );
                break 'fetch_edid;
            }
        };
        manufacturer = edid.base_block.id_manufacturer_name.to_string();
        for descriptor in edid.base_block.descriptors.iter().flatten() {
            match descriptor {
                Descriptor::DisplayProductSerialNumber(s) => {
                    serial_number.clone_from(s);
                }
                Descriptor::DisplayProductName(s) => {
                    name.clone_from(s);
                }
                _ => {}
            }
        }
        if name.is_empty() {
            log::warn!(
                "The display attached to connector {} does not have a product name descriptor",
                connector_id,
            );
        }
        if serial_number.is_empty() {
            log::warn!(
                "The display attached to connector {} does not have a serial number descriptor",
                connector_id,
            );
            serial_number = edid.base_block.id_serial_number.to_string();
        }
        let min_vrr_hz = 'fetch_min_hz: {
            for ext in &edid.extension_blocks {
                if let EdidExtension::CtaV3(cta) = ext {
                    for data_block in &cta.data_blocks {
                        if let CtaDataBlock::VendorAmd(amd) = data_block {
                            break 'fetch_min_hz amd.minimum_refresh_hz as u64;
                        }
                    }
                }
            }
            for desc in &edid.base_block.descriptors {
                if let Some(desc) = desc
                    && let Descriptor::DisplayRangeLimitsAndAdditionalTiming(timings) = desc
                {
                    break 'fetch_min_hz timings.vertical_field_rate_min as u64;
                }
            }
            0
        };
        if min_vrr_hz > 0 {
            vrr_refresh_max_nsec = 1_000_000_000 / min_vrr_hz;
        }
        let cc = &edid.base_block.chromaticity_coordinates;
        let map = |c: u16| F64(c as f64 / 1024.0);
        primaries = Primaries {
            r: (map(cc.red_x), map(cc.red_y)),
            g: (map(cc.green_x), map(cc.green_y)),
            b: (map(cc.blue_x), map(cc.blue_y)),
            wp: (map(cc.white_x), map(cc.white_y)),
        };
        for ext in &edid.extension_blocks {
            if let EdidExtension::CtaV3(cta) = ext {
                for data_block in &cta.data_blocks {
                    match data_block {
                        CtaDataBlock::Colorimetry(c) => {
                            if c.bt2020_rgb {
                                supports_bt2020 = true;
                            }
                        }
                        CtaDataBlock::StaticHdrMetadata(h) => {
                            if h.smpte_st_2084 {
                                supports_pq = true;
                            }
                            if let Some(max) = h.max_luminance {
                                luminance = Some(BackendLuminance {
                                    min: h.min_luminance.unwrap_or(0.0),
                                    max,
                                    max_fall: h.max_luminance.unwrap_or(max),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let output_id = Rc::new(OutputId::new(
        connector_id.to_string(),
        manufacturer,
        name,
        serial_number,
    ));
    let first_mode = info
        .modes
        .first()
        .cloned()
        .map(|m| m.to_backend())
        .unwrap_or_default();
    let persistent = match dev.backend.persistent_display_data.get(&output_id) {
        Some(ds) => {
            if connection != ConnectorStatus::Disconnected {
                log::info!("Reusing desired state for {:?}", output_id);
            }
            ds
        }
        None => {
            let ds = Rc::new(PersistentDisplayData {
                state: RefCell::new(BackendConnectorState {
                    serial: dev.backend.state.backend_connector_state_serials.next(),
                    enabled: true,
                    active: true,
                    mode: first_mode,
                    non_desktop_override: None,
                    vrr: false,
                    tearing: false,
                    format: XRGB8888,
                    color_space: Default::default(),
                    eotf: Default::default(),
                }),
            });
            dev.backend
                .persistent_display_data
                .set(output_id.clone(), ds.clone());
            ds
        }
    };
    let mut desired_state = persistent.state.borrow_mut();
    if desired_state.mode == Mode::default() {
        desired_state.mode = first_mode;
    } else if info
        .modes
        .iter()
        .all(|m| m.to_backend() != desired_state.mode)
    {
        log::warn!("Discarding previously desired mode");
        desired_state.mode = first_mode;
    }
    let non_desktop = props.get("non-desktop")?.value != 0;
    let vrr_capable = match props.get("vrr_capable") {
        Ok(c) => c.value == 1,
        Err(_) => false,
    };
    if !vrr_capable && desired_state.vrr {
        log::warn!("Connector has lost VRR capability");
        desired_state.vrr = false;
    }
    {
        let viable = match desired_state.eotf {
            BackendEotfs::Default => true,
            BackendEotfs::Pq => supports_pq,
        };
        if !viable {
            log::warn!("Discarding previously desired EOTF");
            desired_state.eotf = BackendEotfs::Default;
        }
    }
    {
        let viable = match desired_state.color_space {
            BackendColorSpace::Default => true,
            BackendColorSpace::Bt2020 => supports_bt2020,
        };
        if !viable {
            log::warn!("Discarding previously desired color space");
            desired_state.color_space = BackendColorSpace::Default;
        }
    }
    drop(desired_state);
    let default_properties = create_default_properties(
        &props,
        &[
            ("Broadcast RGB", DefaultValue::Enum("Automatic")),
            ("HDR_SOURCE_METADATA", DefaultValue::Fixed(0)),
            ("Output format", DefaultValue::Enum("Default")),
            ("WRITEBACK_FB_ID", DefaultValue::Fixed(0)),
            ("WRITEBACK_OUT_FENCE_PTR", DefaultValue::Fixed(0)),
            ("content type", DefaultValue::Enum("No Data")),
            ("dither", DefaultValue::Enum("off")),
            ("max bpc", DefaultValue::RangeMax),
        ],
    );
    let hdr_metadata_prop = props
        .get("HDR_OUTPUT_METADATA")
        .map(|p| p.map(|v| DrmBlob(v as _)))
        .ok();
    let mut hdr_metadata = None;
    let mut hdr_metadata_blob_id = DrmBlob::NONE;
    if let Some(p) = &hdr_metadata_prop {
        hdr_metadata_blob_id = p.value;
        hdr_metadata = Some(hdr_output_metadata::from_eotf(
            HDMI_EOTF_TRADITIONAL_GAMMA_SDR,
        ));
        if p.value.is_some() {
            match dev.master.getblob::<hdr_output_metadata>(p.value) {
                Ok(m) => hdr_metadata = Some(m),
                _ => {
                    log::debug!("Could not retrieve hdr output metadata");
                }
            }
        }
    }
    let colorspace_prop = props.get("Colorspace").ok();
    let crtc_id = props.get("CRTC_ID")?.map(|v| DrmCrtc(v as _));
    let drm_state = DrmConnectorState {
        crtc_id: crtc_id.value,
        color_space: colorspace_prop.map(|p| p.value),
        hdr_metadata,
        hdr_metadata_blob_id,
        hdr_metadata_blob: None,
        locked: true,
        fb: DrmFb::NONE,
        fb_idx: 0,
        cursor_fb: DrmFb::NONE,
        cursor_fb_idx: 0,
        cursor_x: 0,
        cursor_y: 0,
        out_fd: None,
        src_w: 0,
        src_h: 0,
        crtc_x: 0,
        crtc_y: 0,
        crtc_w: 0,
        crtc_h: 0,
    };
    Ok(ConnectorDisplayData {
        crtc_id: props.get("CRTC_ID")?.id,
        crtcs,
        first_mode,
        modes: info.modes,
        persistent,
        refresh: 0,
        non_desktop,
        non_desktop_effective: non_desktop,
        vrr_capable,
        _vrr_refresh_max_nsec: vrr_refresh_max_nsec,
        default_properties,
        untyped_properties: props.to_untyped(),
        connection,
        mm_width: info.mm_width,
        mm_height: info.mm_height,
        _subpixel: info.subpixel,
        supports_bt2020,
        supports_pq,
        primaries,
        luminance,
        connector_id,
        output_id,
        colorspace: colorspace_prop.map(|p| p.id),
        hdr_metadata: hdr_metadata_prop.map(|p| p.id),
        drm_state,
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
    let mut possible_planes = BinarySearchMap::new();
    for plane in planes.values() {
        if plane.possible_crtcs.contains(mask) {
            possible_planes.insert(plane.id, plane.clone());
        }
    }
    let props = collect_properties(master, crtc)?;
    let default_properties = create_default_properties(
        &props,
        &[
            ("AMD_CRTC_REGAMMA_TF", DefaultValue::Enum("Default")),
            ("CTM", DefaultValue::Fixed(0)),
            ("DEGAMMA_LUT", DefaultValue::Fixed(0)),
            ("GAMMA_LUT", DefaultValue::Fixed(0)),
            ("OUT_FENCE_PTR", DefaultValue::Fixed(0)),
        ],
    );
    let active = props.get("ACTIVE")?.map(|v| v == 1);
    let mode_id = props.get("MODE_ID")?.map(|v| DrmBlob(v as u32));
    let vrr_enabled = props.get("VRR_ENABLED")?.map(|v| v == 1);
    let out_fence_ptr = props.get("OUT_FENCE_PTR")?;
    let mut mode = None;
    if mode_id.value.is_some() {
        match master.getblob::<drm_mode_modeinfo>(mode_id.value) {
            Ok(m) => mode = Some(m.into()),
            _ => {
                log::debug!("Could not retrieve current mode of connector");
            }
        }
    }
    let state = DrmCrtcState {
        active: active.value,
        mode,
        mode_blob_id: mode_id.value,
        mode_blob: None,
        vrr_enabled: vrr_enabled.value,
        assigned_connector: DrmConnector::NONE,
    };
    Ok(MetalCrtc {
        id: crtc,
        idx,
        master: master.clone(),
        default_properties,
        untyped_properties: RefCell::new(props.to_untyped()),
        lease: Cell::new(None),
        possible_planes,
        connector: Default::default(),
        pending_flip: Default::default(),
        drm_state: RefCell::new(state),
        active: active.id,
        mode_id: mode_id.id,
        vrr_enabled: vrr_enabled.id,
        out_fence_ptr: out_fence_ptr.id,
        sequence: Cell::new(0),
        have_queued_sequence: Cell::new(false),
        needs_vblank_emulation: Cell::new(false),
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
            ));
        }
    };
    let default_properties = create_default_properties(
        &props,
        &[
            ("AMD_PLANE_BLEND_LUT", DefaultValue::Fixed(0)),
            ("AMD_PLANE_BLEND_TF", DefaultValue::Enum("Default")),
            ("AMD_PLANE_CTM", DefaultValue::Fixed(0)),
            ("AMD_PLANE_DEGAMMA_LUT", DefaultValue::Fixed(0)),
            ("AMD_PLANE_HDR_MULT", DefaultValue::Fixed(0)),
            ("AMD_PLANE_LUT3D", DefaultValue::Fixed(0)),
            ("AMD_PLANE_SHAPER_LUT", DefaultValue::Fixed(0)),
            ("AMD_PLANE_SHAPER_TF", DefaultValue::Enum("Default")),
            ("alpha", DefaultValue::RangeMax),
            ("pixel blend mode", DefaultValue::Enum("Pre-multiplied")),
            ("rotation", DefaultValue::Bitmask(&["rotate-0"])),
        ],
    );
    let fb_id = props.get("FB_ID")?.map(|v| DrmFb(v as _));
    let crtc_id = props.get("CRTC_ID")?.map(|v| DrmCrtc(v as _));
    let crtc_x = props.get("CRTC_X")?.map(|v| v as i32);
    let crtc_y = props.get("CRTC_Y")?.map(|v| v as i32);
    let crtc_w = props.get("CRTC_W")?.map(|v| v as i32);
    let crtc_h = props.get("CRTC_H")?.map(|v| v as i32);
    let src_x = props.get("SRC_X")?.map(|v| v as u32);
    let src_y = props.get("SRC_Y")?.map(|v| v as u32);
    let src_w = props.get("SRC_W")?.map(|v| v as u32);
    let src_h = props.get("SRC_H")?.map(|v| v as u32);
    let in_fence_fd = props.get("IN_FENCE_FD")?;
    let state = DrmPlaneState {
        fb_id: fb_id.value,
        src_x: src_x.value,
        src_y: src_y.value,
        src_w: src_w.value,
        src_h: src_h.value,
        assigned_crtc: DrmCrtc::NONE,
        crtc_id: crtc_id.value,
        crtc_x: crtc_x.value,
        crtc_y: crtc_y.value,
        crtc_w: crtc_w.value,
        crtc_h: crtc_h.value,
        buffers: None,
    };
    Ok(MetalPlane {
        id: plane,
        master: master.clone(),
        default_properties,
        untyped_properties: RefCell::new(props.to_untyped()),
        ty,
        possible_crtcs: info.possible_crtcs,
        formats,
        drm_state: RefCell::new(state),
        fb_id: fb_id.id,
        crtc_id: crtc_id.id,
        crtc_x: crtc_x.id,
        crtc_y: crtc_y.id,
        crtc_w: crtc_w.id,
        crtc_h: crtc_h.id,
        src_x: src_x.id,
        src_y: src_y.id,
        src_w: src_w.id,
        src_h: src_h.id,
        in_fence_fd: in_fence_fd.id,
        mode_w: Cell::new(0),
        mode_h: Cell::new(0),
        lease: Cell::new(None),
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
    props: &mut AHashMap<DrmProperty, u64>,
) -> Result<(), DrmError> {
    props.clear();
    for prop in master.get_properties(t)? {
        props.insert(prop.id, prop.value);
    }
    Ok(())
}

struct CollectedProperties {
    props: AHashMap<BString, (DrmPropertyDefinition, u64)>,
}

impl CollectedProperties {
    fn get(&self, name: &str) -> Result<TypedProperty<u64>, DrmError> {
        match self.props.get(name.as_bytes().as_bstr()) {
            Some((def, value)) => Ok(TypedProperty {
                id: def.id,
                value: *value,
            }),
            _ => Err(DrmError::MissingProperty(name.to_string().into_boxed_str())),
        }
    }

    fn to_untyped(&self) -> AHashMap<DrmProperty, u64> {
        let mut res = AHashMap::new();
        for (def, val) in self.props.values() {
            res.insert(def.id, *val);
        }
        res
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TypedProperty<T> {
    pub id: DrmProperty,
    pub value: T,
}

impl<T: Copy> TypedProperty<T> {
    fn map<U, F>(self, f: F) -> TypedProperty<U>
    where
        F: FnOnce(T) -> U,
    {
        TypedProperty {
            id: self.id,
            value: f(self.value),
        }
    }
}

impl MetalBackend {
    pub fn check_render_context(&self, dev: &Rc<MetalDrmDevice>) -> bool {
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
        if let Err(e) = self.handle_drm_change_(&dev) {
            log::error!("Could not handle change of drm device: {}", ErrorFmt(e));
        }
        None
    }

    fn handle_drm_change_(self: &Rc<Self>, dev: &Rc<MetalDrmDeviceData>) -> Result<(), MetalError> {
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
                log::info!(
                    "Removing connector {} from device {}",
                    c.kernel_id.get(),
                    dev.dev.devnode.as_bytes().as_bstr(),
                );
                if let Some(lease_id) = c.lease.get()
                    && let Some(lease) = dev.dev.leases.remove(&lease_id)
                    && !lease.try_revoke()
                {
                    dev.dev.leases_to_break.set(lease_id, lease);
                }
                match c.frontend_state.get() {
                    FrontState::Removed | FrontState::Disconnected => {}
                    FrontState::Connected { .. } | FrontState::Unavailable => {
                        c.send_event(ConnectorEvent::Disconnected);
                    }
                }
                c.send_event(ConnectorEvent::Removed);
            }
        }
        for c in dev.connectors.lock().values() {
            let dd = create_connector_display_data(c.id, &dev.dev);
            let mut dd = match dd {
                Ok(d) => d,
                Err(e) => {
                    log::error!(
                        "Could not update display data for connector: {}",
                        ErrorFmt(e)
                    );
                    continue;
                }
            };
            c.kernel_id.set(dd.connector_id);
            let mut old = c.display.borrow_mut();
            mem::swap(old.deref_mut(), &mut dd);
            old.drm_state = dd.drm_state;
            match c.frontend_state.get() {
                FrontState::Removed | FrontState::Disconnected => {}
                FrontState::Connected { .. } | FrontState::Unavailable => {
                    let mut disconnect = false;
                    // Disconnect if the connector has been disabled.
                    disconnect |= !old.persistent.state.borrow().enabled;
                    // If the connector is connected and switched between being a non-desktop
                    // and desktop device, break leases and disconnect.
                    disconnect |= old.connection == ConnectorStatus::Connected
                        && (c.primary_plane.is_none() != old.non_desktop_effective);
                    if c.lease.is_none() {
                        // If the connector is leased, we have to be careful because DRM is
                        // fickle with sending intermittent disconnected states while the
                        // client performs his setup. Otherwise apply the following rules.

                        // Disconnect if the connector is no longer connected.
                        disconnect |= old.connection != ConnectorStatus::Connected;
                        // Disconnect if the connected monitor changed.
                        disconnect |= old.output_id != dd.output_id;
                    }
                    if disconnect {
                        if let Some(lease_id) = c.lease.get()
                            && let Some(lease) = dev.dev.leases.remove(&lease_id)
                            && !lease.try_revoke()
                        {
                            dev.dev.leases_to_break.set(lease_id, lease);
                        }
                        c.send_event(ConnectorEvent::Disconnected);
                    }
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
            connector.frontend_state.set(FrontState::Disconnected);
            dev.futures.set(c, future);
            dev.connectors.set(c, connector);
        }
        self.init_drm_device(dev)?;
        for connector in dev.connectors.lock().values() {
            if connector.connected() {
                self.start_connector(connector, true);
            }
        }
        Ok(())
    }

    pub fn send_connected(&self, connector: &Rc<MetalConnector>, dd: &ConnectorDisplayData) {
        match connector.frontend_state.get() {
            FrontState::Removed | FrontState::Connected { .. } | FrontState::Unavailable => {
                return;
            }
            FrontState::Disconnected => {}
        }
        let mut prev_mode = None;
        let mut modes = vec![];
        for mode in dd.modes.iter().map(|m| m.to_backend()) {
            if prev_mode.replace(mode) != Some(mode) {
                modes.push(mode);
            }
        }
        let mut eotfs = vec![];
        if dd.supports_pq {
            eotfs.push(BackendEotfs::Pq);
        }
        let mut color_spaces = vec![];
        if dd.supports_bt2020 {
            color_spaces.push(BackendColorSpace::Bt2020);
        }
        let mut state = *dd.persistent.state.borrow();
        state.serial = self.state.backend_connector_state_serials.next();
        connector.send_event(ConnectorEvent::Connected(MonitorInfo {
            modes,
            output_id: dd.output_id.clone(),
            width_mm: dd.mm_width as _,
            height_mm: dd.mm_height as _,
            non_desktop: dd.non_desktop,
            non_desktop_effective: dd.non_desktop_effective,
            vrr_capable: dd.vrr_capable,
            eotfs,
            color_spaces,
            primaries: dd.primaries,
            luminance: dd.luminance,
            state,
        }));
        connector.send_hardware_cursor();
        connector.update_drm_feedback();
        connector.send_formats();
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

        let gbm = match GbmDevice::new(master) {
            Ok(g) => Rc::new(g),
            Err(e) => return Err(MetalError::GbmDevice(e)),
        };

        let gfx = match self.state.create_gfx_context(master, None) {
            Ok(r) => r,
            Err(e) => return Err(MetalError::CreateRenderContex(e)),
        };
        let ctx = Rc::new(MetalRenderContext {
            dev_id: pending.id,
            gfx,
            gbm: gbm.clone(),
            devnode: pending.devnode.clone(),
        });

        let mut is_nvidia = false;
        let mut is_amd = false;
        match gbm.drm.version() {
            Ok(v) => {
                is_nvidia = v.name.contains_str("nvidia");
                is_amd = v.name.contains_str("amdgpu");
                if is_nvidia {
                    log::warn!(
                        "Device {} use the nvidia driver. IN_FENCE_FD will not be used.",
                        pending.devnode.as_bytes().as_bstr(),
                    );
                }
            }
            Err(e) => {
                log::warn!("Could not fetch DRM version information: {}", ErrorFmt(e));
            }
        }

        let dev = Rc::new(MetalDrmDevice {
            backend: self.clone(),
            id: pending.id,
            devnum: pending.devnum,
            devnode: pending.devnode,
            master: master.clone(),
            crtcs,
            encoders,
            planes,
            _min_width: resources.min_width,
            _max_width: resources.max_width,
            _min_height: resources.min_height,
            _max_height: resources.max_height,
            cursor_width,
            cursor_height,
            supports_async_commit: master.supports_async_commit(),
            gbm,
            handle_events: HandleEvents {
                handle_events: Cell::new(None),
            },
            ctx: CloneCell::new(ctx),
            on_change: Default::default(),
            direct_scanout_enabled: Default::default(),
            is_nvidia,
            _is_amd: is_amd,
            lease_ids: Default::default(),
            leases: Default::default(),
            leases_to_break: Default::default(),
            paused: Cell::new(false),
            min_post_commit_margin: Cell::new(DEFAULT_POST_COMMIT_MARGIN),
        });

        let (connectors, futures) = get_connectors(self, &dev, &resources.connectors)?;

        let slf = Rc::new(MetalDrmDeviceData {
            dev: dev.clone(),
            connectors,
            futures,
        });

        self.init_drm_device(&slf)?;

        self.state
            .backend_events
            .push(BackendEvent::NewDrmDevice(dev.clone()));

        for connector in slf.connectors.lock().values() {
            self.state
                .backend_events
                .push(BackendEvent::NewConnector(connector.clone()));
            connector.frontend_state.set(FrontState::Disconnected);
            if connector.connected() {
                self.start_connector(connector, true);
            }
        }

        let drm_handler = self.state.eng.spawn(
            "handle drm events",
            self.clone().handle_drm_events(slf.clone()),
        );
        slf.dev.handle_events.handle_events.set(Some(drm_handler));

        Ok(slf)
    }

    fn update_device_properties(&self, dev: &Rc<MetalDrmDeviceData>) -> Result<(), DrmError> {
        for c in dev.connectors.lock().values() {
            c.update_properties()?;
        }
        for c in dev.dev.crtcs.values() {
            c.update_properties()?;
        }
        for c in dev.dev.planes.values() {
            c.update_properties()?;
        }
        Ok(())
    }
}

impl MetalConnector {
    fn update_properties(&self) -> Result<(), DrmError> {
        let get = |p: &AHashMap<DrmProperty, _>, k: DrmProperty| match p.get(&k) {
            Some(v) => Ok(*v),
            _ => todo!(),
        };
        let master = &self.dev.master;
        let dd = &mut *self.display.borrow_mut();
        collect_untyped_properties(master, self.id, &mut dd.untyped_properties)?;
        let props = &dd.untyped_properties;
        let state = &mut dd.drm_state;
        state.crtc_id = DrmCrtc(get(props, dd.crtc_id)? as _);
        if let Some(cs) = dd.colorspace {
            state.color_space = Some(get(props, cs)?);
        } else {
            state.color_space = None;
        }
        if let Some(meta) = dd.hdr_metadata {
            let id = DrmBlob(get(props, meta)? as _);
            let old = state.hdr_metadata_blob_id;
            state.hdr_metadata_blob_id = id;
            if old != id {
                state.hdr_metadata = None;
                state.hdr_metadata_blob = None;
                if id.is_some() {
                    match master.getblob::<hdr_output_metadata>(id) {
                        Ok(b) => {
                            state.hdr_metadata = Some(b);
                        }
                        Err(e) => {
                            log::error!("Could not fetch hdr_output_metadata: {}", ErrorFmt(e));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl MetalCrtc {
    fn update_properties(&self) -> Result<(), DrmError> {
        let get = |p: &AHashMap<DrmProperty, _>, k: DrmProperty| match p.get(&k) {
            Some(v) => Ok(*v),
            _ => todo!(),
        };
        let master = &self.master;
        let props = &mut *self.untyped_properties.borrow_mut();
        collect_untyped_properties(master, self.id, props)?;
        let state = &mut *self.drm_state.borrow_mut();
        state.active = get(&props, self.active)? != 0;
        state.vrr_enabled = get(&props, self.vrr_enabled)? != 0;
        let id = DrmBlob(get(props, self.mode_id)? as _);
        let old = state.mode_blob_id;
        state.mode_blob_id = id;
        if old != id {
            state.mode = None;
            state.mode_blob = None;
            if id.is_some() {
                match master.getblob::<drm_mode_modeinfo>(id) {
                    Ok(b) => {
                        state.mode = Some(b.into());
                    }
                    Err(e) => {
                        log::error!("Could not fetch drm_mode_modeinfo: {}", ErrorFmt(e));
                    }
                }
            }
        }
        Ok(())
    }
}

impl MetalPlane {
    fn update_properties(&self) -> Result<(), DrmError> {
        let get = |p: &AHashMap<DrmProperty, _>, k: DrmProperty| match p.get(&k) {
            Some(v) => Ok(*v),
            _ => todo!(),
        };
        let props = &mut *self.untyped_properties.borrow_mut();
        collect_untyped_properties(&self.master, self.id, props)?;
        let state = &mut *self.drm_state.borrow_mut();
        state.fb_id = DrmFb(get(props, self.fb_id)? as _);
        state.src_x = get(props, self.src_x)? as _;
        state.src_y = get(props, self.src_y)? as _;
        state.src_w = get(props, self.src_w)? as _;
        state.src_h = get(props, self.src_h)? as _;
        state.crtc_id = DrmCrtc(get(props, self.crtc_id)? as _);
        state.crtc_x = get(props, self.crtc_x)? as _;
        state.crtc_y = get(props, self.crtc_y)? as _;
        state.crtc_w = get(props, self.crtc_w)? as _;
        state.crtc_h = get(props, self.crtc_h)? as _;
        Ok(())
    }
}

impl MetalBackend {
    pub fn resume_drm_device(
        self: &Rc<Self>,
        dev: &Rc<MetalDrmDeviceData>,
    ) -> Result<(), MetalError> {
        for connector in dev.connectors.lock().values() {
            connector.has_damage.fetch_add(1);
            connector.cursor_changed.set(true);
        }
        if let Err(e) = self.update_device_properties(dev) {
            return Err(MetalError::UpdateProperties(e));
        }
        self.init_drm_device(dev)?;
        for connector in dev.connectors.lock().values() {
            if connector.primary_plane.is_some() {
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
            DrmEvent::Sequence {
                time_ns,
                sequence,
                crtc_id,
            } => self.handle_drm_sequence_event(dev, crtc_id, time_ns, sequence),
        }
    }
}

impl MetalCrtc {
    fn update_sequence(&self, new: u64) {
        if self.sequence.replace(new) == new {
            return;
        }
        // nothing
    }

    fn update_u32_sequence(&self, sequence: u32) {
        let old = self.sequence.get();
        let mut new = (old & !(u32::MAX as u64)) | (sequence as u64);
        if new < old {
            new += 1 << u32::BITS;
            if new < old {
                log::warn!("Ignoring nonsensical sequence {sequence} (old = {old})");
                return;
            }
        }
        if new > old + (1 << (u32::BITS - 1)) {
            new = new.saturating_sub(1 << u32::BITS);
            if new < old {
                return;
            }
        }
        self.update_sequence(new);
    }
}

impl MetalBackend {
    fn handle_drm_sequence_event(
        self: &Rc<Self>,
        dev: &Rc<MetalDrmDeviceData>,
        crtc_id: DrmCrtc,
        time_ns: i64,
        sequence: u64,
    ) {
        let crtc = match dev.dev.crtcs.get(&crtc_id) {
            Some(c) => c,
            _ => return,
        };
        crtc.have_queued_sequence.set(false);
        let connector = match crtc.connector.get() {
            Some(c) => c,
            _ => return,
        };
        crtc.update_sequence(sequence);
        crtc.queue_sequence();
        self.state.vblank(connector.connector_id);
        let dd = connector.display.borrow();
        connector
            .next_vblank_nsec
            .set(time_ns as u64 + dd.refresh as u64);
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
        crtc.update_u32_sequence(sequence);
        let wants_present = |c: &MetalConnector| {
            c.has_damage.is_not_zero() || c.cursor_damage.get() || c.cursor_changed.get()
        };
        if let Some(connector) = crtc.pending_flip.take() {
            connector.buffers_idle.set(true);
            if let Some(fb) = connector.next_framebuffer.take() {
                *connector.active_framebuffer.borrow_mut() = Some(fb);
            }
            if wants_present(&connector) && connector.crtc_idle.get() {
                connector.schedule_present();
            }
            let dd = connector.display.borrow();
            let global = self.state.root.outputs.get(&connector.connector_id);
            if let Some(expected) = connector.expected_sequence.take() {
                if connector.vblank_miss_sec.replace(tv_sec) != tv_sec {
                    self.update_post_commit_margin(dev, &connector, &dd, global.as_deref());
                }
                let actual = crtc.sequence.get();
                if expected < actual {
                    connector.vblank_miss_this_sec.fetch_add(1);
                }
            }
            let mut flags = KIND_HW_COMPLETION;
            if connector.presentation_is_sync.get() {
                flags |= KIND_VSYNC;
            }
            if connector.presentation_is_zero_copy.get() {
                flags |= KIND_ZERO_COPY;
            }
            if let Some(g) = &global {
                g.presented(
                    tv_sec as _,
                    tv_usec * 1000,
                    dd.refresh,
                    crtc.sequence.get(),
                    flags,
                    dd.persistent.state.borrow().vrr,
                    dd.drm_state.locked,
                );
            }
        }
        if let Some(connector) = crtc.connector.get() {
            connector.crtc_idle.set(true);
            if !crtc.have_queued_sequence.get() {
                connector.queue_sequence();
            }
            let time_ns = tv_sec as u64 * 1_000_000_000 + tv_usec as u64 * 1000;
            if crtc.needs_vblank_emulation.get() {
                self.handle_drm_sequence_event(dev, crtc_id, time_ns as _, crtc.sequence.get());
            }
            if wants_present(&connector) && connector.buffers_idle.get() {
                connector.schedule_present();
            }
            if connector.presentation_is_sync.get() {
                let dd = connector.display.borrow();
                connector.next_vblank_nsec.set(time_ns + dd.refresh as u64);
            }
        }
    }

    fn update_post_commit_margin(
        &self,
        dev: &MetalDrmDeviceData,
        connector: &MetalConnector,
        dd: &ConnectorDisplayData,
        global: Option<&OutputNode>,
    ) {
        let n_missed = connector.vblank_miss_this_sec.replace(0);
        let old_margin = connector.post_commit_margin.get();
        let new_margin = if n_missed > 0 {
            log::debug!("{}: Missed {n_missed} page flips", connector.kernel_id());
            let refresh = dd.refresh as u64;
            if old_margin >= refresh {
                return;
            }
            let new_margin = (old_margin + POST_COMMIT_MARGIN_DELTA).min(refresh);
            connector.post_commit_margin_decay.reset(new_margin);
            new_margin
        } else {
            let min_margin = dev.dev.min_post_commit_margin.get();
            if min_margin >= connector.post_commit_margin.get() {
                return;
            }
            connector.post_commit_margin_decay.add(min_margin);
            connector.post_commit_margin_decay.get()
        };
        connector.post_commit_margin.set(new_margin);
        if let Some(global) = &global {
            global.flip_margin_ns.set(Some(new_margin));
        }
    }

    fn make_render_device(&self, dev: &MetalDrmDevice, force: bool) {
        if !force
            && let Some(ctx) = self.ctx.get()
            && ctx.dev_id == dev.id
        {
            return;
        }
        let ctx = dev.ctx.get();
        if self.signaled_sync_file.is_none()
            && let Some(sync) = ctx.gfx.sync_obj_ctx()
        {
            match sync.create_signaled_sync_file() {
                Ok(sf) => {
                    self.signaled_sync_file.set(Some(sf));
                }
                Err(e) => {
                    log::warn!("Could not create signaled sync file: {}", ErrorFmt(e));
                }
            }
        }
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
            for connector in dev.connectors.lock().values() {
                connector.send_hardware_cursor();
            }
        }
    }

    fn set_gfx_api(&self, dev: &MetalDrmDevice, api: GfxApi) {
        let old_ctx = dev.ctx.get();
        if old_ctx.gfx.gfx_api() == api {
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
            gbm: old_ctx.gbm.clone(),
            devnode: old_ctx.devnode.clone(),
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
        if let Err(e) = self.init_drm_device(dev) {
            log::error!(
                "Could not initialize drm device {}: {}",
                dev.dev.devnode.as_bytes().as_bstr(),
                ErrorFmt(e),
            );
        }
        for connector in dev.connectors.lock().values() {
            if connector.connected() {
                self.start_connector(connector, false);
            }
        }
    }

    pub fn break_leases(&self, dev: &Rc<MetalDrmDeviceData>) {
        dev.dev
            .leases_to_break
            .lock()
            .retain(|_, lease| !lease.try_revoke());
    }

    fn init_drm_device(&self, dev: &Rc<MetalDrmDeviceData>) -> Result<(), MetalError> {
        self.break_leases(dev);
        enum Quirks {
            DirectScanout,
            NonDefaultFormat,
            NonDefaultMode,
        }
        let mut has_non_default_mode = false;
        let mut has_non_default_format = false;
        let mut has_direct_scanout = false;
        for c in dev.connectors.lock().values() {
            let dd = &*c.display.borrow();
            let state = &*dd.persistent.state.borrow();
            if state.mode != dd.first_mode {
                has_non_default_mode = true;
            }
            if state.format != XRGB8888 {
                has_non_default_format = true;
            }
            if c.direct_scanout_active.get() {
                has_direct_scanout = true;
            }
        }
        let mut quirks = vec![];
        if has_non_default_mode {
            quirks.push(Quirks::NonDefaultMode);
        }
        if has_non_default_format {
            quirks.push(Quirks::NonDefaultFormat);
        }
        if has_direct_scanout {
            quirks.push(Quirks::DirectScanout);
        }
        let apply = |tran: MetalDeviceTransaction| {
            tran.calculate_drm_state()
                .map_err(MetalError::CalculateDrmState)?
                .calculate_change(false, true)
                .map_err(MetalError::CalculateDrmChange)?
                .apply()
                .map_err(MetalError::Modeset)
        };
        let mut disable_non_default_mode = false;
        let mut disable_non_default_format = false;
        let mut disable_direct_scanout = false;
        loop {
            let mut tran = dev.create_transaction();
            for c in dev.connectors.lock().values() {
                let dd = &*c.display.borrow();
                let mut state = *dd.persistent.state.borrow();
                let mut changed_any = false;
                if disable_non_default_format && state.format != XRGB8888 {
                    state.format = XRGB8888;
                    changed_any = true;
                }
                if disable_non_default_mode && state.mode != dd.first_mode {
                    state.mode = dd.first_mode;
                    changed_any = true;
                }
                if changed_any {
                    tran.add(c, state).map_err(MetalError::AddToTransaction)?;
                }
            }
            if disable_direct_scanout {
                tran.disable_direct_scanout();
            }
            let err = match apply(tran) {
                Ok(_) => break,
                Err(e) => e,
            };
            log::error!(
                "Could not initialize DRM device {}: {}",
                dev.dev.devnode.as_bytes().as_bstr(),
                ErrorFmt(&err),
            );
            let Some(q) = quirks.pop() else {
                return Err(err);
            };
            match q {
                Quirks::DirectScanout => {
                    log::info!("Trying to disable direct scanout");
                    disable_direct_scanout = true;
                }
                Quirks::NonDefaultFormat => {
                    log::info!("Trying to disable non XRGB8888 formats");
                    disable_non_default_format = true;
                }
                Quirks::NonDefaultMode => {
                    log::info!("Trying to disable non-default modes");
                    disable_non_default_mode = true;
                }
            }
        }
        Ok(())
    }

    pub fn create_scanout_buffers<const N: usize>(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &'static Format,
        plane_modifiers: &IndexSet<Modifier>,
        width: i32,
        height: i32,
        render_ctx: &Rc<MetalRenderContext>,
        cursor: bool,
    ) -> Result<[RenderBuffer; N], MetalError> {
        let mut blend_buffer = None;
        if !cursor {
            match render_ctx.gfx.acquire_blend_buffer(width, height) {
                Ok(bb) => blend_buffer = Some(bb),
                Err(e) => {
                    log::warn!("Could not create blend buffer: {}", ErrorFmt(e));
                }
            }
        }
        let mut damage_queue = ArrayVec::from(DamageQueue::new::<N>());
        let mut create = || {
            self.create_scanout_buffer(
                dev,
                format,
                plane_modifiers,
                width,
                height,
                render_ctx,
                cursor,
                damage_queue.pop().unwrap(),
                blend_buffer.clone(),
            )
        };
        let mut array = ArrayVec::<_, N>::new();
        for _ in 0..N {
            array.push(create()?);
        }
        if let Some(buffer) = array.first() {
            buffer.damage_full();
        }
        Ok(array.into_inner().unwrap())
    }

    fn create_scanout_buffer(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &'static Format,
        plane_modifiers: &IndexSet<Modifier>,
        width: i32,
        height: i32,
        render_ctx: &Rc<MetalRenderContext>,
        cursor: bool,
        damage_queue: DamageQueue,
        blend_buffer: Option<Rc<dyn GfxBlendBuffer>>,
    ) -> Result<RenderBuffer, MetalError> {
        let mut dev_gfx_write_modifiers = None;
        let mut dev_gfx_read_modifiers = None;
        let mut dev_modifiers_possible = None;
        let mut dev_usage = None;
        let mut dev_modifier = None;
        let mut render_name = None;
        let mut render_gfx_write_modifiers = None;
        let mut render_modifiers_possible = None;
        let mut render_usage = None;
        let mut render_modifier = None;
        self.create_scanout_buffer_(
            dev,
            format,
            plane_modifiers,
            width,
            height,
            render_ctx,
            cursor,
            damage_queue,
            blend_buffer,
            &mut dev_gfx_write_modifiers,
            &mut dev_gfx_read_modifiers,
            &mut dev_modifiers_possible,
            &mut dev_usage,
            &mut dev_modifier,
            &mut render_name,
            &mut render_gfx_write_modifiers,
            &mut render_modifiers_possible,
            &mut render_usage,
            &mut render_modifier,
        )
        .map_err(|kind| ScanoutBufferError {
            dev: dev.devnode.as_bytes().as_bstr().to_string(),
            format,
            plane_modifiers: plane_modifiers.clone(),
            width,
            height,
            cursor,
            dev_gfx_write_modifiers,
            dev_gfx_read_modifiers,
            dev_modifiers_possible,
            dev_usage,
            dev_modifier,
            render_name,
            render_gfx_write_modifiers,
            render_modifiers_possible,
            render_usage,
            render_modifier,
            kind,
        })
        .map_err(Box::new)
        .map_err(MetalError::AllocateScanoutBuffer)
    }

    fn create_scanout_buffer_(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &'static Format,
        plane_modifiers: &IndexSet<Modifier>,
        width: i32,
        height: i32,
        render_ctx: &Rc<MetalRenderContext>,
        cursor: bool,
        damage_queue: DamageQueue,
        blend_buffer: Option<Rc<dyn GfxBlendBuffer>>,
        dbg_dev_gfx_write_modifiers: &mut Option<IndexSet<Modifier>>,
        dbg_dev_gfx_read_modifiers: &mut Option<IndexSet<Modifier>>,
        dbg_dev_modifiers_possible: &mut Option<IndexSet<Modifier>>,
        dbg_dev_usage: &mut Option<u32>,
        dbg_dev_modifier: &mut Option<Modifier>,
        dbg_render_name: &mut Option<String>,
        dbg_render_gfx_write_modifiers: &mut Option<IndexSet<Modifier>>,
        dbg_render_modifiers_possible: &mut Option<IndexSet<Modifier>>,
        dbg_render_usage: &mut Option<u32>,
        dbg_render_modifier: &mut Option<Modifier>,
    ) -> Result<RenderBuffer, ScanoutBufferErrorKind> {
        let dev_ctx = dev.ctx.get();
        let dev_gfx_formats = dev_ctx.gfx.formats();
        let Some(dev_gfx_format) = dev_gfx_formats.get(&format.drm) else {
            return Err(ScanoutBufferErrorKind::SodUnsupportedFormat);
        };
        let send_dev_gfx_write_modifiers = on_drop(|| {
            *dbg_dev_gfx_write_modifiers =
                Some(dev_gfx_format.write_modifiers.keys().copied().collect())
        });
        let possible_modifiers: IndexMap<_, _> = dev_gfx_format
            .write_modifiers
            .iter()
            .filter(|(m, _)| plane_modifiers.contains(*m))
            .map(|(m, v)| (*m, v))
            .collect();
        let send_dev_modifiers_possible = on_drop(|| {
            *dbg_dev_modifiers_possible = Some(possible_modifiers.keys().copied().collect())
        });
        if possible_modifiers.is_empty() {
            return Err(ScanoutBufferErrorKind::SodNoWritableModifier);
        }
        let mut usage = GBM_BO_USE_RENDERING | GBM_BO_USE_SCANOUT;
        if !needs_render_usage(possible_modifiers.values().copied()) {
            usage &= !GBM_BO_USE_RENDERING;
        }
        if cursor {
            usage |= GBM_BO_USE_LINEAR;
        };
        *dbg_dev_usage = Some(usage);
        let dev_bo = dev.gbm.create_bo(
            &self.state.dma_buf_ids,
            width,
            height,
            format,
            possible_modifiers.keys(),
            usage,
        );
        let dev_bo = match dev_bo {
            Ok(b) => b,
            Err(e) => return Err(ScanoutBufferErrorKind::SodBufferAllocation(e)),
        };
        *dbg_dev_modifier = Some(dev_bo.dmabuf().modifier);
        let drm_fb = match dev.master.add_fb(dev_bo.dmabuf(), None) {
            Ok(fb) => Rc::new(fb),
            Err(e) => return Err(ScanoutBufferErrorKind::SodAddfb2(e)),
        };
        let dev_img = match dev_ctx.gfx.clone().dmabuf_img(dev_bo.dmabuf()) {
            Ok(img) => img,
            Err(e) => return Err(ScanoutBufferErrorKind::SodImportSodImage(e)),
        };
        let dev_fb = match dev_img.clone().to_framebuffer() {
            Ok(fb) => fb,
            Err(e) => return Err(ScanoutBufferErrorKind::SodImportFb(e)),
        };
        dev_fb
            .clear(
                AcquireSync::Unnecessary,
                ReleaseSync::None,
                self.state.color_manager.srgb_gamma22(),
            )
            .map_err(ScanoutBufferErrorKind::SodClear)?;
        let render_gfx_formats;
        let render_possible_modifiers: IndexMap<_, _>;
        let mut send_render_dev_name = None;
        let mut send_render_gfx_write_modifiers = None;
        let mut send_dev_gfx_read_modifiers = None;
        let mut send_render_possible_modifiers = None;
        let (dev_tex, render_tex, render_fb, render_bo) = if dev.id == render_ctx.dev_id {
            let render_tex = match dev_img.to_texture() {
                Ok(fb) => fb,
                Err(e) => return Err(ScanoutBufferErrorKind::SodImportSodTexture(e)),
            };
            (None, render_tex, None, None)
        } else {
            send_render_dev_name = Some(on_drop(|| {
                *dbg_render_name = Some(render_ctx.devnode.as_bytes().as_bstr().to_string());
            }));
            // Create a _bridge_ BO in the render device
            render_gfx_formats = render_ctx.gfx.formats();
            let render_gfx_format = match render_gfx_formats.get(&format.drm) {
                None => return Err(ScanoutBufferErrorKind::RenderUnsupportedFormat),
                Some(f) => f,
            };
            send_render_gfx_write_modifiers = Some(on_drop(|| {
                *dbg_render_gfx_write_modifiers =
                    Some(render_gfx_format.write_modifiers.keys().copied().collect())
            }));
            send_dev_gfx_read_modifiers = Some(on_drop(|| {
                *dbg_dev_gfx_read_modifiers = Some(dev_gfx_format.read_modifiers.clone());
            }));
            render_possible_modifiers = render_gfx_format
                .write_modifiers
                .iter()
                .filter(|(m, _)| dev_gfx_format.read_modifiers.contains(*m))
                .map(|(m, v)| (*m, v))
                .collect();
            send_render_possible_modifiers = Some(on_drop(|| {
                *dbg_render_modifiers_possible =
                    Some(render_possible_modifiers.keys().copied().collect())
            }));
            if render_possible_modifiers.is_empty() {
                return Err(ScanoutBufferErrorKind::RenderNoWritableModifier);
            }
            usage = GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR;
            if !needs_render_usage(render_possible_modifiers.values().copied()) {
                usage &= !GBM_BO_USE_RENDERING;
            }
            *dbg_render_usage = Some(usage);
            let render_bo = render_ctx.gbm.create_bo(
                &self.state.dma_buf_ids,
                width,
                height,
                format,
                render_possible_modifiers.keys(),
                usage,
            );
            let render_bo = match render_bo {
                Ok(b) => b,
                Err(e) => return Err(ScanoutBufferErrorKind::RenderBufferAllocation(e)),
            };
            *dbg_render_modifier = Some(render_bo.dmabuf().modifier);
            let render_img = match render_ctx.gfx.clone().dmabuf_img(render_bo.dmabuf()) {
                Ok(img) => img,
                Err(e) => return Err(ScanoutBufferErrorKind::RenderImportImage(e)),
            };
            let render_fb = match render_img.clone().to_framebuffer() {
                Ok(fb) => fb,
                Err(e) => return Err(ScanoutBufferErrorKind::RenderImportFb(e)),
            };
            render_fb
                .clear(
                    AcquireSync::Unnecessary,
                    ReleaseSync::None,
                    self.state.color_manager.srgb_gamma22(),
                )
                .map_err(ScanoutBufferErrorKind::RenderClear)?;
            let render_tex = match render_img.to_texture() {
                Ok(fb) => fb,
                Err(e) => return Err(ScanoutBufferErrorKind::RenderImportRenderTexture(e)),
            };

            // Import the bridge BO into the current device
            let dev_img = match dev_ctx.gfx.clone().dmabuf_img(render_bo.dmabuf()) {
                Ok(img) => img,
                Err(e) => return Err(ScanoutBufferErrorKind::SodImportRenderImage(e)),
            };
            let dev_tex = match dev_img.to_texture() {
                Ok(fb) => fb,
                Err(e) => return Err(ScanoutBufferErrorKind::SodImportRenderTexture(e)),
            };

            (Some(dev_tex), render_tex, Some(render_fb), Some(render_bo))
        };
        send_dev_gfx_write_modifiers.forget();
        send_dev_modifiers_possible.forget();
        send_render_dev_name.map(|o| o.forget());
        send_render_gfx_write_modifiers.map(|o| o.forget());
        send_dev_gfx_read_modifiers.map(|o| o.forget());
        send_render_possible_modifiers.map(|o| o.forget());
        Ok(RenderBuffer {
            width,
            height,
            locked: Cell::new(true),
            format,
            dev_ctx,
            render_ctx: render_ctx.clone(),
            drm: drm_fb,
            damage_queue,
            dev_bo,
            _render_bo: render_bo,
            blend_buffer,
            dev_fb,
            dev_tex,
            render_tex,
            render_fb,
        })
    }

    fn start_connector(&self, connector: &Rc<MetalConnector>, log_mode: bool) {
        let dd = &*connector.display.borrow();
        self.send_connected(connector, dd);
        match connector.frontend_state.get() {
            FrontState::Connected { non_desktop: false } => {}
            FrontState::Connected { non_desktop: true }
            | FrontState::Removed
            | FrontState::Disconnected
            | FrontState::Unavailable => return,
        }
        if log_mode && let Some(crtc) = connector.crtc.get() {
            let state = &*crtc.drm_state.borrow();
            log::info!(
                "Initialized connector {} with mode {:?}",
                dd.connector_id,
                state
                    .mode
                    .as_ref()
                    .map_or(Default::default(), |m| m.to_backend()),
            );
        }
        connector.has_damage.fetch_add(1);
        connector.cursor_changed.set(true);
        connector.schedule_present();
    }
}

#[derive(Debug)]
pub struct RenderBuffer {
    pub width: i32,
    pub height: i32,
    pub locked: Cell<bool>,
    pub format: &'static Format,
    pub dev_ctx: Rc<MetalRenderContext>,
    pub render_ctx: Rc<MetalRenderContext>,
    pub drm: Rc<DrmFramebuffer>,
    pub damage_queue: DamageQueue,
    pub dev_bo: GbmBo,
    pub _render_bo: Option<GbmBo>,
    pub blend_buffer: Option<Rc<dyn GfxBlendBuffer>>,
    // ctx = dev
    // buffer location = dev
    pub dev_fb: Rc<dyn GfxFramebuffer>,
    // ctx = dev
    // buffer location = render
    pub dev_tex: Option<Rc<dyn GfxTexture>>,
    // ctx = render
    // buffer location = render
    pub render_tex: Rc<dyn GfxTexture>,
    // ctx = render
    // buffer location = render
    pub render_fb: Option<Rc<dyn GfxFramebuffer>>,
}

impl RenderBuffer {
    pub fn render_fb(&self) -> Rc<dyn GfxFramebuffer> {
        self.render_fb
            .clone()
            .unwrap_or_else(|| self.dev_fb.clone())
    }

    pub fn copy_to_dev(
        &self,
        cd: &Rc<ColorDescription>,
        sync_file: Option<SyncFile>,
    ) -> Result<Option<SyncFile>, MetalError> {
        let Some(tex) = &self.dev_tex else {
            return Ok(sync_file);
        };
        self.dev_fb
            .copy_texture(
                AcquireSync::Unnecessary,
                ReleaseSync::Explicit,
                cd,
                tex,
                cd,
                None,
                AcquireSync::from_sync_file(sync_file),
                ReleaseSync::None,
                0,
                0,
            )
            .map_err(MetalError::CopyToOutput)
    }

    pub fn damage_full(&self) {
        let dmabuf = self.dev_bo.dmabuf();
        let rect = Rect::new_sized_saturating(0, 0, dmabuf.width, dmabuf.height);
        self.damage_queue.clear_all();
        self.damage_queue.damage(&[rect]);
    }
}

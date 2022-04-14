use {
    crate::{
        async_engine::{AsyncFd, SpawnedFuture},
        backend::{
            BackendEvent, Connector, ConnectorEvent, ConnectorId, ConnectorKernelId, MonitorInfo,
        },
        backends::metal::{DrmId, MetalBackend, MetalError},
        edid::Descriptor,
        format::{Format, XRGB8888},
        render::{Framebuffer, RenderContext},
        state::State,
        utils::{
            bitflags::BitflagsExt, clonecell::CloneCell, debug_fn::debug_fn, errorfmt::ErrorFmt,
            numcell::NumCell, oserror::OsError, syncqueue::SyncQueue,
        },
        video::{
            drm::{
                drm_mode_modeinfo, Change, ConnectorStatus, ConnectorType, DrmBlob, DrmConnector,
                DrmCrtc, DrmEncoder, DrmError, DrmEvent, DrmFramebuffer, DrmMaster, DrmModeInfo,
                DrmObject, DrmPlane, DrmProperty, DrmPropertyDefinition, DrmPropertyType, PropBlob,
                DRM_CLIENT_CAP_ATOMIC, DRM_MODE_ATOMIC_ALLOW_MODESET, DRM_MODE_ATOMIC_NONBLOCK,
                DRM_MODE_PAGE_FLIP_EVENT,
            },
            gbm::{GbmDevice, GBM_BO_USE_RENDERING, GBM_BO_USE_SCANOUT},
            ModifiedFormat, INVALID_MODIFIER,
        },
    },
    ahash::{AHashMap, AHashSet},
    bstr::{BString, ByteSlice},
    std::{
        cell::Cell,
        ffi::CString,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    uapi::c,
};

pub struct PendingDrmDevice {
    pub id: DrmId,
    pub devnum: c::dev_t,
    pub devnode: CString,
}

#[derive(Debug)]
pub struct MetalDrmDeviceStatic {
    pub id: DrmId,
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
    pub gbm: GbmDevice,
    pub egl: Rc<RenderContext>,
    pub async_fd: AsyncFd,
    pub handle_events: HandleEvents,
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
pub struct MetalDrmDevice {
    pub dev: Rc<MetalDrmDeviceStatic>,
    pub connectors: AHashMap<DrmConnector, Rc<MetalConnector>>,
}

#[derive(Debug)]
pub struct MetalConnector {
    pub id: DrmConnector,
    pub master: Rc<DrmMaster>,

    pub connector_id: ConnectorId,

    pub crtcs: AHashMap<DrmCrtc, Rc<MetalCrtc>>,
    pub modes: Vec<DrmModeInfo>,
    pub mode: CloneCell<Option<Rc<DrmModeInfo>>>,

    pub monitor_manufacturer: String,
    pub monitor_name: String,
    pub monitor_serial_number: String,

    pub events: SyncQueue<ConnectorEvent>,

    pub buffers: CloneCell<Option<Rc<[RenderBuffer; 2]>>>,
    pub next_buffer: NumCell<usize>,

    pub connector_type: ConnectorType,
    pub connector_type_id: u32,

    pub connection: ConnectorStatus,
    pub mm_width: u32,
    pub mm_height: u32,
    pub subpixel: u32,

    pub primary_plane: CloneCell<Option<Rc<MetalPlane>>>,

    pub crtc_id: MutableProperty<DrmCrtc>,
    pub crtc: CloneCell<Option<Rc<MetalCrtc>>>,

    pub on_change: OnChange,
}

#[derive(Default)]
pub struct OnChange {
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
}

impl Debug for OnChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.on_change.get() {
            None => f.write_str("None"),
            Some(_) => f.write_str("Some"),
        }
    }
}

impl Connector for MetalConnector {
    fn id(&self) -> ConnectorId {
        self.connector_id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        ConnectorKernelId {
            ty: self.connector_type,
            idx: self.connector_type_id,
        }
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.on_change.set(Some(cb));
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
pub struct MetalPlane {
    pub id: DrmPlane,
    pub master: Rc<DrmMaster>,

    pub ty: PlaneType,

    pub possible_crtcs: u32,
    pub formats: AHashMap<u32, &'static Format>,

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
    state: &State,
    dev: &MetalDrmDeviceStatic,
    ids: &[DrmConnector],
) -> Result<AHashMap<DrmConnector, Rc<MetalConnector>>, DrmError> {
    let mut connectors = AHashMap::new();
    for connector in ids {
        match create_connector(state, *connector, dev) {
            Ok(e) => {
                connectors.insert(e.id, Rc::new(e));
            }
            Err(e) => return Err(DrmError::CreateConnector(Box::new(e))),
        }
    }
    Ok(connectors)
}

fn create_connector(
    state: &State,
    connector: DrmConnector,
    dev: &MetalDrmDeviceStatic,
) -> Result<MetalConnector, DrmError> {
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
    let connector_type = ConnectorType::from_drm(info.connector_type);
    let mut name = String::new();
    let mut manufacturer = String::new();
    let mut serial_number = String::new();
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
        for descriptor in &edid.base_block.descriptors {
            if let Some(d) = descriptor {
                match d {
                    Descriptor::DisplayProductSerialNumber(s) => {
                        serial_number = s.clone();
                    }
                    Descriptor::DisplayProductName(s) => {
                        name = s.clone();
                    }
                    _ => {}
                }
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
    Ok(MetalConnector {
        id: connector,
        master: dev.master.clone(),
        connector_id: state.connector_ids.next(),
        crtcs,
        mode: CloneCell::new(info.modes.first().cloned().map(Rc::new)),
        monitor_manufacturer: manufacturer,
        monitor_name: name,
        monitor_serial_number: serial_number,
        events: Default::default(),
        modes: info.modes,
        buffers: Default::default(),
        next_buffer: Default::default(),
        connector_type,
        connector_type_id: info.connector_type_id,
        connection,
        mm_width: info.mm_width,
        mm_height: info.mm_height,
        subpixel: info.subpixel,
        primary_plane: Default::default(),
        crtc_id: props.get("CRTC_ID")?.map(|v| DrmCrtc(v as _)),
        crtc: Default::default(),
        on_change: Default::default(),
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
    let mut formats = AHashMap::new();
    for format in info.format_types {
        if let Some(f) = crate::format::formats().get(&format) {
            formats.insert(format, *f);
        } else {
            // log::warn!(
            //     "{:?} supports unknown format '{:?}'",
            //     plane,
            //     crate::format::debug(format)
            // );
        }
    }
    let props = collect_properties(master, plane)?;
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

impl MetalBackend {
    pub fn create_drm_device(
        self: &Rc<Self>,
        pending: PendingDrmDevice,
        master: &Rc<DrmMaster>,
    ) -> Result<Rc<MetalDrmDevice>, MetalError> {
        if let Err(e) = master.set_client_cap(DRM_CLIENT_CAP_ATOMIC, 2) {
            return Err(MetalError::AtomicModesetting(e));
        }
        let resources = master.get_resources()?;

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
            Ok(g) => g,
            Err(e) => return Err(MetalError::GbmDevice(e)),
        };
        let egl = match RenderContext::from_drm_device(master) {
            Ok(r) => Rc::new(r),
            Err(e) => return Err(MetalError::CreateRenderContex(e)),
        };
        let async_fd = match self.state.eng.fd(master.fd()) {
            Ok(f) => f,
            Err(e) => return Err(MetalError::CreateDrmAsyncFd(e)),
        };

        let dev = Rc::new(MetalDrmDeviceStatic {
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
            gbm,
            egl: egl.clone(),
            async_fd,
            handle_events: HandleEvents {
                handle_events: Cell::new(None),
            },
        });

        let connectors = get_connectors(&self.state, &dev, &resources.connectors)?;

        let slf = Rc::new(MetalDrmDevice { dev, connectors });

        self.init_drm_device(&slf)?;

        for connector in slf.connectors.values() {
            self.state
                .backend_events
                .push(BackendEvent::NewConnector(connector.clone()));
            if connector.connection == ConnectorStatus::Connected {
                if connector.primary_plane.get().is_none() {
                    log::error!(
                        "Connector {}-{} is connected but does not have a primary plane",
                        connector.connector_type,
                        connector.connector_type_id
                    );
                    continue;
                }
                let mut prev_mode = None;
                let mut modes = vec![];
                for mode in connector.modes.iter().map(|m| m.to_backend()) {
                    if prev_mode.replace(mode) != Some(mode) {
                        modes.push(mode);
                    }
                }
                connector
                    .events
                    .push(ConnectorEvent::Connected(MonitorInfo {
                        modes,
                        manufacturer: connector.monitor_manufacturer.clone(),
                        product: connector.monitor_name.clone(),
                        serial_number: connector.monitor_serial_number.clone(),
                        initial_mode: connector.mode.get().unwrap().to_backend(),
                        width_mm: connector.mm_width as _,
                        height_mm: connector.mm_height as _,
                    }));
                self.start_connector(connector);
            }
        }

        let handler = self
            .state
            .eng
            .spawn(self.clone().handle_drm_events(slf.clone()));
        slf.dev.handle_events.handle_events.set(Some(handler));

        self.state.set_render_ctx(&egl);

        Ok(slf)
    }

    fn update_device_properties(&self, dev: &Rc<MetalDrmDevice>) -> Result<(), DrmError> {
        let get = |p: &AHashMap<DrmProperty, _>, k: DrmProperty| match p.get(&k) {
            Some(v) => Ok(*v),
            _ => todo!(),
        };
        let master = &dev.dev.master;
        for c in dev.connectors.values() {
            let props = collect_untyped_properties(master, c.id)?;
            c.crtc_id
                .value
                .set(DrmCrtc(get(&props, c.crtc_id.id)? as _));
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

    pub fn resume_drm_device(self: &Rc<Self>, dev: &Rc<MetalDrmDevice>) -> Result<(), MetalError> {
        if let Err(e) = self.update_device_properties(dev) {
            return Err(MetalError::UpdateProperties(e));
        }
        self.init_drm_device(dev)?;
        for connector in dev.connectors.values() {
            if connector.primary_plane.get().is_some() {
                self.present(connector);
            }
        }
        Ok(())
    }

    async fn handle_drm_events(self: Rc<Self>, dev: Rc<MetalDrmDevice>) {
        loop {
            if let Err(e) = dev.dev.async_fd.readable().await {
                log::error!("Could not register the DRM fd for reading: {}", ErrorFmt(e));
                break;
            }
            loop {
                match dev.dev.master.event() {
                    Ok(Some(e)) => self.handle_drm_event(e, &dev),
                    Ok(None) => break,
                    Err(e) => {
                        log::error!("Could not read DRM event: {}", ErrorFmt(e));
                        return;
                    }
                }
            }
        }
    }

    fn handle_drm_event(self: &Rc<Self>, event: DrmEvent, dev: &Rc<MetalDrmDevice>) {
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
        dev: &Rc<MetalDrmDevice>,
        crtc_id: DrmCrtc,
        _tv_sec: u32,
        _tv_usec: u32,
        _sequence: u32,
    ) {
        let crtc = match dev.dev.crtcs.get(&crtc_id) {
            Some(c) => c,
            _ => return,
        };
        let connector = match crtc.connector.get() {
            Some(c) => c,
            _ => return,
        };
        self.present(&connector);
    }

    fn reset_planes(&self, dev: &MetalDrmDevice, changes: &mut Change) {
        for plane in dev.dev.planes.values() {
            plane.crtc_id.value.set(DrmCrtc::NONE);
            changes.change_object(plane.id, |c| {
                c.change(plane.crtc_id.id, 0);
                c.change(plane.fb_id, 0);
                c.change(plane.in_fence_fd, -1i32 as u64);
            })
        }
    }

    fn reset_connectors_and_crtcs(&self, dev: &MetalDrmDevice, changes: &mut Change) {
        for connector in dev.connectors.values() {
            connector.primary_plane.set(None);
            connector.crtc.set(None);
            connector.crtc_id.value.set(DrmCrtc::NONE);
            changes.change_object(connector.id, |c| {
                c.change(connector.crtc_id.id, 0);
            })
        }
        for crtc in dev.dev.crtcs.values() {
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

    fn init_drm_device(&self, dev: &Rc<MetalDrmDevice>) -> Result<(), MetalError> {
        let mut flags = 0;
        let mut changes = dev.dev.master.change();
        if !self.can_use_current_drm_mode(dev) {
            log::warn!("Cannot use existing connector configuration. Trying to perform modeset.");
            flags = DRM_MODE_ATOMIC_ALLOW_MODESET;
            self.reset_connectors_and_crtcs(dev, &mut changes);
            for connector in dev.connectors.values() {
                if let Err(e) = self.assign_connector_crtc(connector, &mut changes) {
                    log::error!("Could not assign a crtc: {}", ErrorFmt(e));
                }
            }
        }
        self.reset_planes(dev, &mut changes);
        for connector in dev.connectors.values() {
            if let Err(e) = self.assign_connector_plane(dev, connector, &mut changes) {
                log::error!("Could not assign a plane: {}", ErrorFmt(e));
            }
        }
        if let Err(e) = changes.commit(flags, 0) {
            return Err(MetalError::Modeset(e));
        }
        Ok(())
    }

    fn can_use_current_drm_mode(&self, dev: &Rc<MetalDrmDevice>) -> bool {
        let mut used_crtcs = AHashSet::new();
        let mut used_planes = AHashSet::new();

        for connector in dev.connectors.values() {
            if connector.connection != ConnectorStatus::Connected {
                if connector.crtc_id.value.get().is_some() {
                    log::debug!("Connector is not connected but has an assigned crtc");
                    return false;
                }
                continue;
            }
            let crtc_id = connector.crtc_id.value.get();
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
            let mode = match connector.mode.get() {
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
            if !modes_equal(&mode, &current_mode) {
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
        format: &ModifiedFormat,
        width: i32,
        height: i32,
    ) -> Result<[RenderBuffer; 2], MetalError> {
        let create = || self.create_scanout_buffer(dev, format, width, height);
        Ok([create()?, create()?])
    }

    fn create_scanout_buffer(
        &self,
        dev: &Rc<MetalDrmDevice>,
        format: &ModifiedFormat,
        width: i32,
        height: i32,
    ) -> Result<RenderBuffer, MetalError> {
        let bo = dev.dev.gbm.create_bo(
            width,
            height,
            format,
            GBM_BO_USE_RENDERING | GBM_BO_USE_SCANOUT,
        );
        let bo = match bo {
            Ok(b) => b,
            Err(e) => return Err(MetalError::ScanoutBuffer(e)),
        };
        let drm_fb = match dev.dev.master.add_fb(bo.dmabuf()) {
            Ok(fb) => Rc::new(fb),
            Err(e) => return Err(MetalError::Framebuffer(e)),
        };
        let egl_fb = match dev.dev.egl.dmabuf_fb(bo.dmabuf()) {
            Ok(fb) => fb,
            Err(e) => return Err(MetalError::ImportFb(e)),
        };
        egl_fb.clear();
        Ok(RenderBuffer {
            drm: drm_fb,
            egl: egl_fb,
        })
    }

    fn assign_connector_crtc(
        &self,
        connector: &Rc<MetalConnector>,
        changes: &mut Change,
    ) -> Result<(), MetalError> {
        if connector.connection != ConnectorStatus::Connected {
            return Ok(());
        }
        let crtc = 'crtc: {
            for crtc in connector.crtcs.values() {
                if crtc.connector.get().is_none() {
                    break 'crtc crtc.clone();
                }
            }
            return Err(MetalError::NoCrtcForConnector);
        };
        let mode = match connector.mode.get() {
            Some(m) => m,
            _ => return Err(MetalError::NoModeForConnector),
        };
        let mode_blob = mode.create_blob(&connector.master)?;
        changes.change_object(connector.id, |c| {
            c.change(connector.crtc_id.id, crtc.id.0 as _);
        });
        changes.change_object(crtc.id, |c| {
            c.change(crtc.active.id, 1);
            c.change(crtc.mode_id.id, mode_blob.id().0 as _);
        });
        connector.crtc.set(Some(crtc.clone()));
        connector.crtc_id.value.set(crtc.id);
        crtc.connector.set(Some(connector.clone()));
        crtc.active.value.set(true);
        crtc.mode_id.value.set(mode_blob.id());
        crtc.mode_blob.set(Some(Rc::new(mode_blob)));
        Ok(())
    }

    fn assign_connector_plane(
        &self,
        dev: &Rc<MetalDrmDevice>,
        connector: &Rc<MetalConnector>,
        changes: &mut Change,
    ) -> Result<(), MetalError> {
        let crtc = match connector.crtc.get() {
            Some(c) => c,
            _ => return Ok(()),
        };
        let mode = match connector.mode.get() {
            Some(m) => m,
            _ => {
                log::error!("Connector has a crtc assigned but no mode");
                return Ok(());
            }
        };
        let primary_plane = 'primary_plane: {
            for plane in crtc.possible_planes.values() {
                if plane.ty == PlaneType::Primary
                    && plane.crtc_id.value.get().is_none()
                    && plane.formats.contains_key(&XRGB8888.drm)
                {
                    break 'primary_plane plane.clone();
                }
            }
            return Err(MetalError::NoPrimaryPlaneForConnector);
        };
        connector.buffers.set(None);
        let buffers = match connector.buffers.get() {
            Some(b) => b,
            None => {
                let format = ModifiedFormat {
                    format: XRGB8888,
                    modifier: INVALID_MODIFIER,
                };
                Rc::new(self.create_scanout_buffers(
                    dev,
                    &format,
                    mode.hdisplay as _,
                    mode.vdisplay as _,
                )?)
            }
        };
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
        primary_plane.crtc_id.value.set(crtc.id);
        primary_plane.crtc_x.value.set(0);
        primary_plane.crtc_y.value.set(0);
        primary_plane.crtc_w.value.set(mode.hdisplay as _);
        primary_plane.crtc_h.value.set(mode.vdisplay as _);
        primary_plane.src_x.value.set(0);
        primary_plane.src_y.value.set(0);
        primary_plane.src_w.value.set((mode.hdisplay as u32) << 16);
        primary_plane.src_h.value.set((mode.vdisplay as u32) << 16);
        connector.buffers.set(Some(buffers));
        connector.primary_plane.set(Some(primary_plane.clone()));
        Ok(())
    }

    fn start_connector(&self, connector: &Rc<MetalConnector>) {
        let mode = connector.mode.get().unwrap();
        log::info!(
            "Initialized connector {}-{} with mode {:?}",
            connector.connector_type,
            connector.connector_type_id,
            mode
        );
        self.present(connector);
    }

    pub fn present(&self, connector: &Rc<MetalConnector>) {
        let crtc = match connector.crtc.get() {
            Some(crtc) => crtc,
            _ => return,
        };
        if !crtc.active.value.get() {
            return;
        }
        let buffers = match connector.buffers.get() {
            None => return,
            Some(b) => b,
        };
        let plane = match connector.primary_plane.get() {
            Some(p) => p,
            _ => return,
        };
        let buffer = &buffers[connector.next_buffer.fetch_add(1) % buffers.len()];
        if let Some(node) = self.state.root.outputs.get(&connector.connector_id) {
            buffer
                .egl
                .render(&*node, &self.state, Some(node.global.pos.get()));
        }
        let mut changes = connector.master.change();
        changes.change_object(plane.id, |c| {
            c.change(plane.fb_id, buffer.drm.id().0 as _);
        });
        if let Err(e) = changes.commit(DRM_MODE_ATOMIC_NONBLOCK | DRM_MODE_PAGE_FLIP_EVENT, 0) {
            match e {
                DrmError::Atomic(OsError(c::EACCES)) => {
                    log::debug!("Could not perform atomic commit, likely because we're no longer the DRM master");
                }
                _ => log::error!("Could not set plane framebuffer: {}", ErrorFmt(e)),
            }
        }
    }
}

#[derive(Debug)]
pub struct RenderBuffer {
    drm: Rc<DrmFramebuffer>,
    egl: Rc<Framebuffer>,
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

use crate::async_engine::{AsyncFd, SpawnedFuture};
use crate::backend::{BackendEvent, Output, OutputId};
use crate::drm::drm::{
    drm_mode_modeinfo, Change, ConnectorStatus, ConnectorType, DrmBlob, DrmConnector, DrmCrtc,
    DrmEncoder, DrmError, DrmEvent, DrmFramebuffer, DrmMaster, DrmModeInfo, DrmObject, DrmPlane,
    DrmProperty, DrmPropertyDefinition, DrmPropertyType, PropBlob, DRM_CLIENT_CAP_ATOMIC,
    DRM_MODE_ATOMIC_ALLOW_MODESET, DRM_MODE_ATOMIC_NONBLOCK, DRM_MODE_PAGE_FLIP_EVENT,
};
use crate::drm::gbm::{GbmDevice, GBM_BO_USE_RENDERING, GBM_BO_USE_SCANOUT};
use crate::drm::{ModifiedFormat, INVALID_MODIFIER};
use crate::format::{Format, XRGB8888};
use crate::metal::{DrmId, MetalBackend, MetalError};
use crate::render::{Framebuffer, RenderContext};
use crate::utils::bitflags::BitflagsExt;
use crate::utils::oserror::OsError;
use crate::{CloneCell, ErrorFmt, NumCell, State};
use ahash::{AHashMap, AHashSet};
use bstr::{BString, ByteSlice};
use std::cell::Cell;
use std::ffi::CString;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::time::Instant;
use uapi::c;

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

    pub active: Cell<bool>,

    pub output_id: OutputId,

    pub crtcs: AHashMap<DrmCrtc, Rc<MetalCrtc>>,
    pub modes: Vec<DrmModeInfo>,
    pub mode: CloneCell<Option<Rc<DrmModeInfo>>>,

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

impl Output for MetalConnector {
    fn id(&self) -> OutputId {
        self.output_id
    }

    fn removed(&self) -> bool {
        false
    }

    fn width(&self) -> i32 {
        match self.mode.get() {
            Some(m) => m.hdisplay as _,
            _ => 0,
        }
    }

    fn height(&self) -> i32 {
        match self.mode.get() {
            Some(m) => m.vdisplay as _,
            _ => 0,
        }
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
    Ok(MetalConnector {
        id: connector,
        master: dev.master.clone(),
        active: Cell::new(false),
        output_id: state.output_ids.next(),
        crtcs,
        mode: CloneCell::new(info.modes.first().cloned().map(|m| Rc::new(m))),
        modes: info.modes,
        buffers: Default::default(),
        next_buffer: Default::default(),
        connector_type: info.connector_type.into(),
        connector_type_id: info.connector_type_id,
        connection: info.connection.into(),
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
    pub fn creat_drm_device(
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
            if connector.primary_plane.get().is_some() {
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

    fn reuse_primary_planes(&self, dev: &MetalDrmDevice) -> AHashSet<DrmPlane> {
        let mut crtc_primary_planes = AHashMap::new();
        for connector in dev.connectors.values() {
            connector.active.set(false);
            connector.primary_plane.set(None);
            if let Some(crtc) = connector.crtc.get() {
                crtc_primary_planes.insert(crtc.id, vec![]);
            }
        }
        for plane in dev.dev.planes.values() {
            if plane.ty == PlaneType::Primary {
                if let Some(ncp) = crtc_primary_planes.get_mut(&plane.crtc_id.value.get()) {
                    ncp.push(plane.clone());
                }
            }
        }
        let mut reuse_possible = true;
        for planes in crtc_primary_planes.values() {
            if planes.len() > 1 {
                reuse_possible = false;
                break;
            }
            if let Some(plane) = planes.first() {
                if !plane.formats.contains_key(&XRGB8888.drm) {
                    reuse_possible = false;
                    break;
                }
            }
        }
        let mut preserve = AHashSet::new();
        if !reuse_possible {
            log::warn!("Not reusing primary planes");
            return preserve;
        }
        for connector in dev.connectors.values() {
            if let Some(planes) = crtc_primary_planes.get(&connector.crtc_id.value.get()) {
                if let Some(plane) = planes.first() {
                    connector.primary_plane.set(Some(plane.clone()));
                    preserve.insert(plane.id);
                }
            }
        }
        preserve
    }

    fn reset_planes(&self, dev: &MetalDrmDevice, changes: &mut Change, preserve: &AHashSet<DrmPlane>) {
        for plane in dev.dev.planes.values() {
            if preserve.contains(&plane.id) {
                continue;
            }
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
        let preserve = self.reuse_primary_planes(dev);
        self.reset_planes(dev, &mut changes, &preserve);
        {
            let mut connector: Vec<_> = dev.connectors.values().collect();
            connector.sort_by_key(|k| {
                if k.primary_plane.get().is_some() {
                    0
                } else {
                    1
                }
            });
            for connector in connector {
                if let Err(e) = self.assign_connector_plane(dev, connector, &mut changes) {
                    log::error!("Could not assign a plane: {}", ErrorFmt(e));
                }
            }
        }
        for connector in dev.connectors.values() {
            if !connector.active.get() {
                connector.primary_plane.set(None);
            }
        }
        let mut start = Instant::now();
        if let Err(e) = changes.commit(flags, 0) {
            return Err(MetalError::Modeset(e));
        }
        log::info!("commit 2: {:?}", start.elapsed());
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
            &format,
            GBM_BO_USE_RENDERING | GBM_BO_USE_SCANOUT,
        );
        let bo = match bo {
            Ok(b) => b,
            Err(e) => return Err(MetalError::ScanoutBuffer(e)),
        };
        let drm_fb = match dev.dev.master.add_fb(bo.dma()) {
            Ok(fb) => Rc::new(fb),
            Err(e) => return Err(MetalError::Framebuffer(e)),
        };
        let egl_fb = match dev.dev.egl.dmabuf_fb(bo.dma()) {
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
        let mut primary_plane = connector.primary_plane.get();
        if primary_plane.is_none() {
            for plane in crtc.possible_planes.values() {
                if plane.ty == PlaneType::Primary
                    && plane.crtc_id.value.get().is_none()
                    && plane.formats.contains_key(&XRGB8888.drm)
                {
                    primary_plane = Some(plane.clone());
                    break;
                }
            }
        }
        let primary_plane = match primary_plane {
            Some(p) => p,
            _ => return Err(MetalError::NoPrimaryPlaneForConnector),
        };
        let format = ModifiedFormat {
            format: XRGB8888,
            modifier: INVALID_MODIFIER,
        };
        let buffers = match connector.buffers.get() {
            Some(b) => b,
            None => Rc::new(self.create_scanout_buffers(
                dev,
                &format,
                mode.hdisplay as _,
                mode.vdisplay as _,
            )?),
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
        connector.active.set(true);
        Ok(())
    }

    fn start_connector(&self, connector: &Rc<MetalConnector>) {
        let mode = connector.mode.get().unwrap();
        self.state
            .backend_events
            .push(BackendEvent::NewOutput(connector.clone()));
        log::info!(
            "Initialized connector {}-{} with mode {:?}",
            connector.connector_type,
            connector.connector_type_id,
            mode
        );
        self.present(connector);
    }

    fn present(&self, connector: &Rc<MetalConnector>) {
        let buffers = match connector.buffers.get() {
            None => return,
            Some(b) => b,
        };
        let plane = match connector.primary_plane.get() {
            Some(p) => p,
            _ => return,
        };
        let buffer = &buffers[connector.next_buffer.fetch_add(1) % buffers.len()];
        if let Some(node) = self.state.root.outputs.get(&connector.output_id) {
            buffer
                .egl
                .render(&*node, &self.state, Some(node.position.get()));
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
    true && a.clock == b.clock
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

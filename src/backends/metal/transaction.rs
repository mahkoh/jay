use {
    crate::{
        allocator::BufferObject,
        backend::{
            BackendColorSpace, BackendConnectorState, BackendTransferFunction, Connector,
            ConnectorEvent,
            transaction::{
                BackendAppliedConnectorTransaction, BackendConnectorTransaction,
                BackendConnectorTransactionError, BackendPreparedConnectorTransaction,
            },
        },
        backends::metal::video::{
            FrontState, MetalConnector, MetalCrtc, MetalDrmDeviceData, MetalPlane, PlaneType,
            RenderBuffer,
        },
        format::{ARGB8888, Format},
        gfx_api::{AcquireSync, ReleaseSync, SyncFile},
        utils::{
            binary_search_map::BinarySearchMap, cell_ext::CellExt, errorfmt::ErrorFmt, rc_eq::rc_eq,
        },
        video::drm::{
            Change, ConnectorStatus, DRM_MODE_ATOMIC_ALLOW_MODESET, DrmBlob, DrmConnector, DrmCrtc,
            DrmFb, DrmModeInfo, DrmObject, DrmPlane, PropBlob, hdr_output_metadata,
        },
    },
    arrayvec::ArrayVec,
    bstr::ByteSlice,
    isnt::std_1::collections::IsntHashMap2Ext,
    std::{any::Any, cell::Cell, mem, rc::Rc, slice},
    uapi::c,
};

const LEVEL: log::Level = log::Level::Debug;

#[derive(Default, Clone, Debug)]
pub struct DrmPlaneState {
    pub fb_id: DrmFb,
    pub src_x: u32,
    pub src_y: u32,
    pub src_w: u32,
    pub src_h: u32,
    pub assigned_crtc: DrmCrtc,
    pub crtc_id: DrmCrtc,
    pub crtc_x: i32,
    pub crtc_y: i32,
    pub crtc_w: i32,
    pub crtc_h: i32,
    pub buffers: Option<Rc<[RenderBuffer; 2]>>,
}

#[derive(Default, Clone)]
pub struct DrmCrtcState {
    pub active: bool,
    pub mode: Option<DrmModeInfo>,
    pub mode_blob_id: DrmBlob,
    pub mode_blob: Option<Rc<PropBlob>>,
    pub vrr_enabled: bool,
    pub assigned_connector: DrmConnector,
}

#[derive(Default, Clone, Debug)]
pub struct DrmConnectorState {
    pub crtc_id: DrmCrtc,
    pub color_space: Option<u64>,
    pub hdr_metadata: Option<hdr_output_metadata>,
    pub hdr_metadata_blob_id: DrmBlob,
    pub hdr_metadata_blob: Option<Rc<PropBlob>>,
    pub locked: bool,
    pub fb: DrmFb,
    pub fb_idx: u64,
    pub cursor_fb: DrmFb,
    pub cursor_fb_idx: u64,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub out_fd: Option<SyncFile>,
    pub src_w: u32,
    pub src_h: u32,
    pub crtc_x: i32,
    pub crtc_y: i32,
    pub crtc_w: i32,
    pub crtc_h: i32,
}

struct PlaneConfig {
    obj: Rc<MetalPlane>,
    new: DrmPlaneState,
    changed: ArrayVec<Rc<Cell<bool>>, 4>,
}

struct CrtcConfig {
    obj: Rc<MetalCrtc>,
    new: DrmCrtcState,
    changed: ArrayVec<Rc<Cell<bool>>, 2>,
}

struct ConnectorConfig {
    obj: Rc<MetalConnector>,
    new: DrmConnectorState,
    state: BackendConnectorState,
    requested: bool,
    changed: Rc<Cell<bool>>,
}

const SIZE: usize = 16;

struct TransactionCommon {
    dev: Rc<MetalDrmDeviceData>,
    planes: BinarySearchMap<DrmPlane, PlaneConfig, SIZE>,
    crtcs: BinarySearchMap<DrmCrtc, CrtcConfig, SIZE>,
    connectors: BinarySearchMap<DrmConnector, ConnectorConfig, SIZE>,
}

pub struct MetalDeviceTransaction {
    common: TransactionCommon,
    allow_direct_scanout: bool,
}

pub struct MetalDeviceTransactionWithDrmState {
    common: TransactionCommon,
}

pub struct MetalDeviceTransactionWithChange {
    common: TransactionCommon,
    change: Change,
}

pub struct MetalDeviceAppliedTransaction {
    rollback: MetalDeviceTransactionWithDrmState,
}

impl MetalConnector {
    pub fn create_transaction(
        &self,
    ) -> Result<MetalDeviceTransaction, BackendConnectorTransactionError> {
        let Some(dev) = self.backend.device_holder.drm_devices.get(&self.dev.devnum) else {
            return Err(BackendConnectorTransactionError::MissingDrmDevice(
                self.kernel_id(),
            ));
        };
        Ok(dev.create_transaction())
    }
}

impl MetalDrmDeviceData {
    pub fn create_transaction(self: &Rc<Self>) -> MetalDeviceTransaction {
        let mut tran = MetalDeviceTransaction {
            common: TransactionCommon {
                dev: self.clone(),
                planes: Default::default(),
                crtcs: Default::default(),
                connectors: Default::default(),
            },
            allow_direct_scanout: true,
        };
        for plane in self.dev.planes.values() {
            if plane.lease.is_some() {
                continue;
            }
            tran.common.planes.insert(
                plane.id,
                PlaneConfig {
                    obj: plane.clone(),
                    new: plane.drm_state.borrow().clone(),
                    changed: Default::default(),
                },
            );
        }
        for crtc in self.dev.crtcs.values() {
            if crtc.lease.is_some() {
                continue;
            }
            tran.common.crtcs.insert(
                crtc.id,
                CrtcConfig {
                    obj: crtc.clone(),
                    new: crtc.drm_state.borrow().clone(),
                    changed: Default::default(),
                },
            );
        }
        for connector in self.connectors.lock().values() {
            if connector.lease.is_some() {
                continue;
            }
            let dd = &*connector.display.borrow();
            tran.common.connectors.insert(
                connector.id,
                ConnectorConfig {
                    obj: connector.clone(),
                    new: dd.drm_state.clone(),
                    state: *dd.persistent.state.borrow(),
                    requested: false,
                    changed: Default::default(),
                },
            );
        }
        tran
    }
}

const CURSOR_FORMAT: &Format = ARGB8888;

#[derive(Default, Debug)]
struct CrtcPlanes {
    primary: DrmPlane,
    cursor: DrmPlane,
}

impl MetalDeviceTransaction {
    pub fn add(
        &mut self,
        connector: &Rc<MetalConnector>,
        state: BackendConnectorState,
    ) -> Result<(), BackendConnectorTransactionError> {
        let Some(config) = self.common.connectors.get_mut(&connector.id) else {
            if self.common.dev.connectors.contains(&connector.id) {
                return Err(BackendConnectorTransactionError::LeasedConnector(
                    connector.kernel_id(),
                ));
            }
            return Err(BackendConnectorTransactionError::UnknownConnector(
                connector.kernel_id(),
            ));
        };
        config.state = state;
        config.requested = true;
        Ok(())
    }

    pub fn disable_direct_scanout(&mut self) {
        self.allow_direct_scanout = false;
    }

    pub fn calculate_drm_state(
        mut self,
    ) -> Result<MetalDeviceTransactionWithDrmState, BackendConnectorTransactionError> {
        let mut unused_crtcs = BinarySearchMap::<_, _, SIZE>::new();
        let mut unused_planes = BinarySearchMap::<_, _, SIZE>::new();
        let mut crtc_planes = BinarySearchMap::<_, _, SIZE>::new();
        let mut sync_files = vec![];
        let slf = &mut self.common;
        for (_, crtc) in &mut slf.crtcs {
            crtc_planes.insert(crtc.obj.id, CrtcPlanes::default());
            unused_crtcs.insert(crtc.obj.id, ());
        }
        for (_, connector) in &slf.connectors {
            unused_crtcs.remove(&connector.new.crtc_id);
            if let Some(crtc) = slf.crtcs.get_mut(&connector.new.crtc_id)
                && crtc.changed.is_empty()
            {
                crtc.changed.push(connector.changed.clone());
            }
        }
        for (_, plane) in &mut slf.planes {
            if let Some(crtc) = slf.crtcs.get_mut(&plane.new.assigned_crtc) {
                plane.changed.extend(crtc.changed.iter().cloned());
            }
            if plane.new.crtc_id.is_some() {
                plane.new.assigned_crtc = plane.new.crtc_id;
            }
            macro_rules! discard_plane {
                () => {
                    unused_planes.insert(plane.obj.id, ());
                    plane.new.crtc_id = DrmCrtc::NONE;
                    plane.new.assigned_crtc = DrmCrtc::NONE;
                };
            }
            if plane.new.assigned_crtc.is_none() {
                discard_plane!();
                continue;
            }
            if unused_crtcs.contains(&plane.new.assigned_crtc) {
                discard_plane!();
                continue;
            }
            let Some(crtc_planes) = crtc_planes.get_mut(&plane.new.assigned_crtc) else {
                discard_plane!();
                continue;
            };
            let field = match plane.obj.ty {
                PlaneType::Overlay => {
                    discard_plane!();
                    continue;
                }
                PlaneType::Primary => &mut crtc_planes.primary,
                PlaneType::Cursor => &mut crtc_planes.cursor,
            };
            if field.is_some() {
                discard_plane!();
                continue;
            }
            *field = plane.obj.id;
        }
        let render_ctx = slf.dev.dev.backend.ctx.get();
        let dev_ctx = slf.dev.dev.ctx.get();
        for connector in slf.connectors.values_mut() {
            let state = &connector.state;
            let dd = &*connector.obj.display.borrow();
            if !state.enabled
                || dd.connection != ConnectorStatus::Connected
                || state.non_desktop_override.unwrap_or(dd.non_desktop)
            {
                if connector.new.crtc_id.is_some() {
                    unused_crtcs.insert(connector.new.crtc_id, ());
                    if let Some(crtc) = slf.crtcs.get(&connector.new.crtc_id) {
                        let planes = crtc_planes.get_mut(&crtc.obj.id).unwrap();
                        for plane in [&mut planes.primary, &mut planes.cursor] {
                            if plane.is_some() {
                                unused_planes.insert(*plane, ());
                                let plane = slf.planes.get_mut(plane).unwrap();
                                plane.new.crtc_id = DrmCrtc::NONE;
                                plane.new.assigned_crtc = DrmCrtc::NONE;
                            }
                        }
                        *planes = CrtcPlanes::default();
                    }
                }
                connector.new = DrmConnectorState::default();
                continue;
            }
            if connector.new.crtc_id.is_none() {
                let crtc_id = 'crtc_id: {
                    for (crtc, _) in &dd.crtcs {
                        if unused_crtcs.contains(crtc) {
                            break 'crtc_id crtc;
                        }
                    }
                    return Err(BackendConnectorTransactionError::NoCrtcForConnector(
                        connector.obj.kernel_id(),
                    ));
                };
                unused_crtcs.remove(crtc_id);
                connector.new.crtc_id = *crtc_id;
            }
            let crtc = slf.crtcs.get_mut(&connector.new.crtc_id).unwrap();
            crtc.new.active = state.active;
            crtc.new.assigned_connector = connector.obj.id;
            crtc.changed.push(connector.changed.clone());
            let crtc_planes = crtc_planes.get_mut(&crtc.obj.id).unwrap();
            let plane_not_supports_format = |plane: &MetalPlane| {
                let format = match plane.ty {
                    PlaneType::Overlay => unreachable!(),
                    PlaneType::Primary => state.format,
                    PlaneType::Cursor => CURSOR_FORMAT,
                };
                plane.formats.not_contains_key(&format.drm)
            };
            for plane in [&mut crtc_planes.primary, &mut crtc_planes.cursor] {
                macro_rules! discard_plane {
                    () => {
                        unused_planes.insert(*plane, ());
                        *plane = DrmPlane::NONE;
                    };
                }
                if plane.is_none() {
                    discard_plane!();
                    continue;
                }
                let plane = slf.planes.get(plane).unwrap();
                if plane_not_supports_format(&plane.obj) {
                    discard_plane!();
                    continue;
                }
            }
            for (primary, plane) in [
                (true, &mut crtc_planes.primary),
                (false, &mut crtc_planes.cursor),
            ] {
                if plane.is_some() {
                    continue;
                }
                let ty = match primary {
                    true => PlaneType::Primary,
                    false => PlaneType::Cursor,
                };
                for (_, p) in &crtc.obj.possible_planes {
                    if p.ty != ty {
                        continue;
                    }
                    if unused_planes.not_contains(&p.id) {
                        continue;
                    }
                    if plane_not_supports_format(p) {
                        continue;
                    }
                    *plane = p.id;
                    unused_planes.remove(&p.id);
                }
            }
            if crtc_planes.primary.is_none() {
                return Err(
                    BackendConnectorTransactionError::NoPrimaryPlaneForConnector(
                        connector.obj.kernel_id(),
                    ),
                );
            }
            let mode = 'mode: {
                let Some(mode) = dd.modes.iter().find(|m| m.to_backend() == state.mode) else {
                    return Err(BackendConnectorTransactionError::UnsupportedMode(
                        connector.obj.kernel_id(),
                        state.mode,
                    ));
                };
                if let Some(old) = &crtc.new.mode
                    && modes_equal(old, mode)
                {
                    break 'mode mode.clone();
                }
                crtc.new.mode = Some(mode.clone());
                let blob = slf
                    .dev
                    .dev
                    .master
                    .create_blob(&mode.to_raw())
                    .map_err(BackendConnectorTransactionError::CreateModeBlob)?;
                crtc.new.mode_blob_id = blob.id();
                crtc.new.mode_blob = Some(Rc::new(blob));
                mode.clone()
            };
            for plane in [crtc_planes.primary, crtc_planes.cursor] {
                if plane.is_none() {
                    continue;
                }
                let plane = slf.planes.get_mut(&plane).unwrap();
                plane.new.assigned_crtc = crtc.obj.id;
                plane.changed.extend(crtc.changed.iter().cloned());
                let (x, y, width, height, format, old_buffers);
                match plane.obj.ty {
                    PlaneType::Overlay => unreachable!(),
                    PlaneType::Primary => {
                        (x, y) = (0, 0);
                        width = mode.hdisplay as i32;
                        height = mode.vdisplay as i32;
                        format = state.format;
                        old_buffers = connector.obj.buffers.get();
                    }
                    PlaneType::Cursor => {
                        x = connector.new.cursor_x;
                        y = connector.new.cursor_y;
                        width = connector.obj.dev.cursor_width as i32;
                        height = connector.obj.dev.cursor_height as i32;
                        format = CURSOR_FORMAT;
                        old_buffers = connector.obj.cursor_buffers.get();
                    }
                };
                plane.new.buffers = old_buffers.clone();
                plane.new.src_x = 0;
                plane.new.src_y = 0;
                plane.new.src_w = (width as u32) << 16;
                plane.new.src_h = (height as u32) << 16;
                plane.new.crtc_x = x;
                plane.new.crtc_y = y;
                plane.new.crtc_w = width;
                plane.new.crtc_h = height;
                if let Some(b) = &plane.new.buffers {
                    'discard: {
                        macro_rules! discard {
                            () => {
                                plane.new.buffers = None;
                                break 'discard;
                            };
                        }
                        if b[0].width != width || b[0].height != height || b[0].format != format {
                            discard!();
                        }
                        let Some(render_ctx) = &render_ctx else {
                            discard!();
                        };
                        if !rc_eq(render_ctx, &b[0].render_ctx) {
                            discard!();
                        }
                        if !rc_eq(&dev_ctx, &b[0].dev_ctx) {
                            discard!();
                        }
                        let modifiers = &plane.obj.formats.get(&format.drm).unwrap().modifiers;
                        for b in &**b {
                            if !modifiers.contains(&b.dev_bo.dmabuf().modifier) {
                                discard!();
                            }
                        }
                    }
                }
                let mut new_buffers = None;
                let current_buffers = match &plane.new.buffers {
                    Some(b) => b.clone(),
                    None => {
                        let modifiers = &plane.obj.formats.get(&format.drm).unwrap().modifiers;
                        connector.changed.set(true);
                        let buffers = slf
                            .dev
                            .dev
                            .backend
                            .create_scanout_buffers(
                                &slf.dev.dev,
                                format,
                                modifiers,
                                width,
                                height,
                                &slf.dev.dev.ctx.get(),
                                plane.obj.ty == PlaneType::Cursor,
                            )
                            .map_err(|e| {
                                BackendConnectorTransactionError::AllocateScanoutBuffers(
                                    connector.obj.kernel_id(),
                                    Box::new(e),
                                )
                            })?;
                        let buffers = Rc::new(buffers);
                        plane.new.buffers = Some(buffers.clone());
                        new_buffers = Some(buffers.clone());
                        buffers
                    }
                };
                let (fb_id, fb_idx) = match plane.obj.ty {
                    PlaneType::Overlay => unreachable!(),
                    PlaneType::Primary => (connector.new.fb, &mut connector.new.fb_idx),
                    PlaneType::Cursor => {
                        (connector.new.cursor_fb, &mut connector.new.cursor_fb_idx)
                    }
                };
                plane.new.crtc_id = DrmCrtc::NONE;
                plane.new.fb_id = DrmFb::NONE;
                if plane.obj.ty == PlaneType::Primary || fb_id.is_some() {
                    plane.new.crtc_id = crtc.obj.id;
                    let locked = slf.dev.dev.backend.state.lock.locked.get();
                    let may_show_current_fb = !crtc.new.active
                        || connector.new.locked
                        || !locked
                        || plane.obj.ty != PlaneType::Primary;
                    if plane.obj.ty == PlaneType::Primary
                        && connector.obj.direct_scanout_active.get()
                        && self.allow_direct_scanout
                        && may_show_current_fb
                    {
                        plane.new.fb_id = fb_id;
                        macro_rules! copy {
                            ($field:ident) => {
                                plane.new.$field = connector.new.$field;
                            };
                        }
                        copy!(src_w);
                        copy!(src_h);
                        copy!(crtc_x);
                        copy!(crtc_y);
                        copy!(crtc_w);
                        copy!(crtc_h);
                    } else if current_buffers.iter().any(|b| b.drm.id() == fb_id)
                        && may_show_current_fb
                    {
                        plane.new.fb_id = fb_id;
                    } else if let Some(new_buffers) = &new_buffers {
                        let new_buffer = &new_buffers[0];
                        plane.new.fb_id = new_buffer.drm.id();
                        *fb_idx = 0;
                        let cd = connector.obj.color_description.get();
                        let res = if let Some(prev) = &old_buffers
                            && let Some(prev) = prev.iter().find(|b| b.drm.id() == fb_id)
                            && rc_eq(&new_buffer.dev_ctx, &prev.dev_ctx)
                            && may_show_current_fb
                        {
                            let src = prev.dev_tex.as_ref().unwrap_or(&prev.render_tex);
                            let dst = &new_buffer.dev_fb;
                            dst.copy_texture(
                                AcquireSync::Unnecessary,
                                ReleaseSync::Explicit,
                                &cd,
                                src,
                                &cd,
                                None,
                                AcquireSync::Unnecessary,
                                ReleaseSync::Explicit,
                                0,
                                0,
                            )
                        } else {
                            new_buffer.dev_fb.clear(
                                AcquireSync::Unnecessary,
                                ReleaseSync::Explicit,
                                &cd,
                            )
                        };
                        match res {
                            Ok(sf) => sync_files.extend(sf),
                            Err(e) => {
                                log::warn!("Could not copy from old buffer: {}", ErrorFmt(e));
                            }
                        }
                    } else {
                        if may_show_current_fb {
                            let idx = *fb_idx % current_buffers.len() as u64;
                            plane.new.fb_id = current_buffers[idx as usize].drm.id();
                        } else {
                            let idx = (*fb_idx + 1) % current_buffers.len() as u64;
                            *fb_idx = idx;
                            let buffer = &current_buffers[idx as usize];
                            plane.new.fb_id = buffer.drm.id();
                            if !buffer.locked.get() {
                                if !connector.obj.buffers_idle.get()
                                    && let Some(fd) = &connector.new.out_fd
                                {
                                    log::log!(LEVEL, "waiting for CRTC sync file before blanking");
                                    let mut pollfd = c::pollfd {
                                        fd: fd.raw(),
                                        events: c::POLLIN,
                                        revents: 0,
                                    };
                                    let res = uapi::poll(slice::from_mut(&mut pollfd), -1);
                                    if let Err(e) = res {
                                        log::warn!(
                                            "Could not wait for CRTC sync file to become readable: {}",
                                            ErrorFmt(e),
                                        );
                                    }
                                }
                                buffer.damage_full();
                                let cd = connector.obj.color_description.get();
                                let res = buffer.dev_fb.clear(
                                    AcquireSync::Unnecessary,
                                    ReleaseSync::Explicit,
                                    &cd,
                                );
                                match res {
                                    Ok(sf) => {
                                        buffer.locked.set(true);
                                        sync_files.extend(sf);
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Could not black out old buffer: {}",
                                            ErrorFmt(e),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                if plane.obj.ty == PlaneType::Primary {
                    macro_rules! copy {
                        ($field:ident) => {
                            connector.new.$field = plane.new.$field;
                        };
                    }
                    copy!(src_w);
                    copy!(src_h);
                    copy!(crtc_x);
                    copy!(crtc_y);
                    copy!(crtc_w);
                    copy!(crtc_h);
                }
            }
            if state.vrr && !dd.vrr_capable {
                return Err(BackendConnectorTransactionError::NotVrrCapable(
                    connector.obj.kernel_id(),
                ));
            }
            crtc.new.vrr_enabled = state.vrr;
            if state.tearing && !slf.dev.dev.supports_async_commit {
                return Err(BackendConnectorTransactionError::TearingNotSupported(
                    connector.obj.kernel_id(),
                ));
            }
            match state.color_space {
                BackendColorSpace::Default => {}
                BackendColorSpace::Bt2020 => {
                    if !dd.supports_bt2020 {
                        return Err(BackendConnectorTransactionError::ColorSpaceNotSupported(
                            connector.obj.kernel_id(),
                            state.color_space,
                        ));
                    }
                }
            }
            match state.transfer_function {
                BackendTransferFunction::Default => {}
                BackendTransferFunction::Pq => {
                    if !dd.supports_pq {
                        return Err(
                            BackendConnectorTransactionError::TransferFunctionNotSupported(
                                connector.obj.kernel_id(),
                                state.transfer_function,
                            ),
                        );
                    }
                }
            }
            if let Some(cs) = &mut connector.new.color_space {
                *cs = state.color_space.to_drm();
            }
            if dd.hdr_metadata.is_some() {
                let new = if state.transfer_function == BackendTransferFunction::Default {
                    None
                } else {
                    Some(hdr_output_metadata::from_eotf(
                        state.transfer_function.to_drm(),
                    ))
                };
                if connector.new.hdr_metadata != new {
                    if let Some(new) = &new {
                        let blob = slf
                            .dev
                            .dev
                            .master
                            .create_blob(new)
                            .map_err(BackendConnectorTransactionError::CreateHdrMetadataBlob)?;
                        connector.new.hdr_metadata_blob_id = blob.id();
                        connector.new.hdr_metadata_blob = Some(Rc::new(blob));
                    } else {
                        connector.new.hdr_metadata_blob_id = DrmBlob::NONE;
                        connector.new.hdr_metadata_blob = None;
                    }
                    connector.new.hdr_metadata = new;
                } else if new.is_none() {
                    connector.new.hdr_metadata_blob_id = DrmBlob::NONE;
                    connector.new.hdr_metadata_blob = None;
                }
            }
        }
        for (crtc, _) in &unused_crtcs {
            if let Some(crtc) = slf.crtcs.get_mut(crtc) {
                crtc.new = DrmCrtcState::default();
            }
        }
        for (plane, _) in &unused_planes {
            if let Some(plane) = slf.planes.get_mut(plane) {
                plane.new = DrmPlaneState::default();
            }
        }
        for sf in sync_files {
            let mut pollfd = c::pollfd {
                fd: sf.0.raw(),
                events: c::POLLIN,
                revents: 0,
            };
            let res = uapi::poll(slice::from_mut(&mut pollfd), -1);
            if let Err(e) = res {
                log::warn!(
                    "Could not wait for sync file to become readable: {}",
                    ErrorFmt(e)
                );
            }
        }
        Ok(MetalDeviceTransactionWithDrmState {
            common: self.common,
        })
    }
}

macro_rules! log_change {
    ($o:expr, $n:expr, $field:ident) => {
        log::log!(
            LEVEL,
            "changed {}: {:?} -> {:?}",
            stringify!($field),
            $o.$field,
            $n.$field
        );
    };
}

impl MetalDeviceTransactionWithDrmState {
    pub fn calculate_change(
        mut self,
        test: bool,
        reset_default_properties: bool,
    ) -> Result<MetalDeviceTransactionWithChange, BackendConnectorTransactionError> {
        macro_rules! reset_default_properties {
            ($c:expr, $props:expr, $defaults:expr $(,)?) => {{
                if reset_default_properties {
                    let props = $props;
                    for dp in $defaults {
                        let old = props.get(&dp.prop).copied().unwrap_or_default();
                        let new = dp.value;
                        if old != new {
                            log::log!(LEVEL, "changed {}: {old} -> {new}", dp.name);
                            $c.change(dp.prop, new);
                        }
                    }
                }
            }};
        }

        let slf = &mut self.common;
        let mut c = slf.dev.dev.master.change();
        for (_, connector) in &mut slf.connectors {
            let dd = &*connector.obj.display.borrow();
            let n = &mut connector.new;
            let o = &dd.drm_state;
            let changed = c.change_object(connector.obj.id, |c| {
                if n.crtc_id != o.crtc_id {
                    log_change!(o, n, crtc_id);
                    c.change(dd.crtc_id, n.crtc_id);
                }
                if let Some(prop) = &dd.colorspace
                    && let Some(new_cs) = n.color_space
                    && let Some(old_cs) = o.color_space
                    && new_cs != old_cs
                {
                    log_change!(o, n, color_space);
                    c.change(*prop, new_cs);
                }
                if let Some(prop) = &dd.hdr_metadata
                    && n.hdr_metadata_blob_id != o.hdr_metadata_blob_id
                {
                    log_change!(o, n, hdr_metadata_blob_id);
                    c.change(*prop, n.hdr_metadata_blob_id);
                }
                reset_default_properties!(c, &dd.untyped_properties, &dd.default_properties);
            });
            if changed {
                connector.changed.set(true);
            }
            log::log!(
                LEVEL,
                "connector {:?} (crtc {:?}) {}changed",
                connector.obj.id,
                connector.new.crtc_id,
                if changed { "" } else { "un" },
            );
        }
        for (_, crtc) in &mut slf.crtcs {
            let n = &mut crtc.new;
            let o = &*crtc.obj.drm_state.borrow();
            let changed = c.change_object(crtc.obj.id, |c| {
                if n.active != o.active {
                    log_change!(o, n, active);
                    c.change(crtc.obj.active, n.active);
                }
                if n.vrr_enabled != o.vrr_enabled {
                    log_change!(o, n, vrr_enabled);
                    c.change(crtc.obj.vrr_enabled, n.vrr_enabled);
                }
                if n.mode_blob_id != o.mode_blob_id {
                    log_change!(o, n, mode_blob_id);
                    c.change(crtc.obj.mode_id, n.mode_blob_id);
                }
                reset_default_properties!(
                    c,
                    &*crtc.obj.untyped_properties.borrow(),
                    &crtc.obj.default_properties,
                );
            });
            if changed {
                log::log!(LEVEL, "crtc {:?} changed", crtc.obj.id);
                crtc.changed.iter().for_each(|c| c.set(true));
            }
        }
        for (_, plane) in &mut slf.planes {
            let n = &mut plane.new;
            let o = &*plane.obj.drm_state.borrow();
            let changed = c.change_object(plane.obj.id, |c| {
                if n.fb_id != o.fb_id {
                    log_change!(o, n, fb_id);
                    c.change(plane.obj.fb_id, n.fb_id);
                    c.change(plane.obj.in_fence_fd, -1i32);
                }
                if n.crtc_id != o.crtc_id {
                    log_change!(o, n, crtc_id);
                    c.change(plane.obj.crtc_id, n.crtc_id);
                }
                macro_rules! change {
                    ($field:ident) => {
                        if n.$field != o.$field {
                            log_change!(o, n, $field);
                            c.change(plane.obj.$field, n.$field);
                        }
                    };
                }
                change!(src_x);
                change!(src_y);
                change!(src_w);
                change!(src_h);
                change!(crtc_x);
                change!(crtc_y);
                change!(crtc_w);
                change!(crtc_h);
                reset_default_properties!(
                    c,
                    &*plane.obj.untyped_properties.borrow(),
                    &plane.obj.default_properties,
                );
            });
            if changed {
                plane.changed.iter().for_each(|c| c.set(true));
            }
            log::log!(
                LEVEL,
                "plane {:?} (crtc {:?}) (ty {:?}) {}changed",
                plane.obj.id,
                plane.new.crtc_id,
                plane.obj.ty,
                if changed { "" } else { "un" },
            );
        }
        log::log!(
            LEVEL,
            "device {} {}changed",
            self.common.dev.dev.devnode.to_bytes().as_bstr(),
            if c.is_not_empty() { "" } else { "un" },
        );
        if test {
            c.test(DRM_MODE_ATOMIC_ALLOW_MODESET)
                .map_err(BackendConnectorTransactionError::AtomicTestFailed)?;
        }
        Ok(MetalDeviceTransactionWithChange {
            common: self.common,
            change: c,
        })
    }
}

impl MetalDeviceTransactionWithChange {
    pub fn apply(
        mut self,
    ) -> Result<MetalDeviceAppliedTransaction, BackendConnectorTransactionError> {
        let c = &self.change;
        if c.is_not_empty()
            && let Err(e) = c.commit(0, 0)
        {
            log::log!(
                LEVEL,
                "Transaction of device {} could not be applied without modeset: {}",
                self.common.dev.dev.devnode.to_bytes().as_bstr(),
                ErrorFmt(e),
            );
            log::log!(LEVEL, "Performing modeset");
            c.commit(DRM_MODE_ATOMIC_ALLOW_MODESET, 0)
                .map_err(BackendConnectorTransactionError::AtomicCommitFailed)?;
        }
        let slf = &mut self.common;
        let mut crtc_planes = BinarySearchMap::<_, _, SIZE>::new();
        for (_, crtc) in &mut slf.crtcs {
            crtc.obj.connector.set(None);
            if crtc.new.assigned_connector.is_some() {
                let connector = slf
                    .dev
                    .connectors
                    .get(&crtc.new.assigned_connector)
                    .unwrap();
                crtc.obj.connector.set(Some(connector));
                crtc_planes.insert(crtc.obj.id, CrtcPlanes::default());
            }
        }
        for (_, plane) in &mut slf.planes {
            if plane.new.assigned_crtc.is_some() {
                let crtc = slf.crtcs.get(&plane.new.assigned_crtc).unwrap();
                let mode = crtc.new.mode.as_ref().unwrap();
                plane.obj.mode_w.set(mode.hdisplay as _);
                plane.obj.mode_h.set(mode.vdisplay as _);
                let planes = crtc_planes.get_mut(&plane.new.assigned_crtc).unwrap();
                match plane.obj.ty {
                    PlaneType::Overlay => unreachable!(),
                    PlaneType::Primary => planes.primary = plane.obj.id,
                    PlaneType::Cursor => planes.cursor = plane.obj.id,
                }
            }
        }
        for (_, connector) in &mut slf.connectors {
            if !connector.changed.get() {
                continue;
            }
            connector.obj.version.fetch_add(1);
            if connector.new.crtc_id.is_none() {
                connector.obj.crtc.set(None);
                connector.obj.primary_plane.set(None);
                connector.obj.cursor_plane.set(None);
                connector.obj.buffers.set(None);
                connector.obj.cursor_buffers.set(None);
            } else {
                let crtc = slf.crtcs.get(&connector.new.crtc_id).unwrap();
                crtc.obj.connector.set(Some(connector.obj.clone()));
                connector.obj.crtc.set(Some(crtc.obj.clone()));
                connector.obj.crtc_idle.set(crtc.obj.pending_flip.is_none());
                let planes = crtc_planes.get(&crtc.obj.id).unwrap();
                for (primary, plane) in [(true, planes.primary), (false, planes.cursor)] {
                    if plane.is_none() {
                        match primary {
                            true => {
                                connector.obj.primary_plane.set(None);
                                connector.obj.buffers.set(None);
                                connector.new.fb = DrmFb::NONE;
                            }
                            false => {
                                connector.obj.cursor_plane.set(None);
                                connector.obj.cursor_buffers.set(None);
                                connector.new.cursor_fb = DrmFb::NONE;
                            }
                        }
                        continue;
                    }
                    let plane = slf.planes.get(&plane).unwrap();
                    match plane.obj.ty {
                        PlaneType::Overlay => unreachable!(),
                        PlaneType::Primary => {
                            connector.obj.primary_plane.set(Some(plane.obj.clone()));
                            connector.obj.buffers.set(plane.new.buffers.clone());
                            connector.new.fb = plane.new.fb_id;
                        }
                        PlaneType::Cursor => {
                            connector.obj.cursor_plane.set(Some(plane.obj.clone()));
                            connector.obj.cursor_buffers.set(plane.new.buffers.clone());
                            connector.new.cursor_fb = plane.new.fb_id;
                        }
                    }
                }
            }
        }
        for (_, crtc) in &mut slf.crtcs {
            let o = &mut *crtc.obj.drm_state.borrow_mut();
            mem::swap(o, &mut crtc.new);
        }
        for (_, plane) in &mut slf.planes {
            let o = &mut *plane.obj.drm_state.borrow_mut();
            mem::swap(o, &mut plane.new);
        }
        for (_, connector) in &mut slf.connectors {
            let is_connected;
            let is_non_desktop;
            {
                let dd = &mut *connector.obj.display.borrow_mut();
                mem::swap(&mut dd.drm_state, &mut connector.new);
                mem::swap(&mut *dd.persistent.state.borrow_mut(), &mut connector.state);
                dd.update_cached_fields(&slf.dev.dev);
                is_non_desktop = dd.non_desktop_effective;
                is_connected = dd.connection == ConnectorStatus::Connected;
            }
            if connector.obj.crtc.is_some() {
                if connector.changed.get() {
                    if let Some(buffers) = connector.obj.buffers.get() {
                        buffers[0].damage_full();
                    }
                    connector.obj.has_damage.fetch_add(1);
                    connector.obj.cursor_damage.set(true);
                    if connector.obj.buffers_idle.get() && connector.obj.crtc_idle.get() {
                        connector.obj.schedule_present();
                    }
                }
                match connector.obj.frontend_state.get() {
                    FrontState::Removed | FrontState::Unavailable => {}
                    FrontState::Disconnected => connector.obj.send_connected(),
                    FrontState::Connected { non_desktop: false } => {
                        if connector.changed.get() || connector.requested {
                            connector.obj.send_hardware_cursor();
                            connector.obj.send_formats();
                            connector.obj.update_drm_feedback();
                            connector.obj.send_state();
                        }
                    }
                    FrontState::Connected { non_desktop: true } => {
                        connector.obj.send_event(ConnectorEvent::Disconnected);
                        connector.obj.send_connected();
                    }
                }
            } else if is_connected && is_non_desktop {
                match connector.obj.frontend_state.get() {
                    FrontState::Removed
                    | FrontState::Unavailable
                    | FrontState::Connected { non_desktop: true } => {}
                    FrontState::Disconnected => connector.obj.send_connected(),
                    FrontState::Connected { non_desktop: false } => {
                        connector.obj.send_event(ConnectorEvent::Disconnected);
                        connector.obj.send_connected();
                    }
                }
            } else {
                match connector.obj.frontend_state.get() {
                    FrontState::Removed | FrontState::Unavailable | FrontState::Disconnected => {}
                    FrontState::Connected { .. } => {
                        connector.obj.send_event(ConnectorEvent::Disconnected);
                    }
                }
            }
        }
        Ok(MetalDeviceAppliedTransaction {
            rollback: MetalDeviceTransactionWithDrmState {
                common: self.common,
            },
        })
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

impl MetalDeviceAppliedTransaction {
    pub fn rollback(self) -> Result<(), BackendConnectorTransactionError> {
        self.rollback.calculate_change(false, false)?.apply()?;
        Ok(())
    }
}

impl BackendConnectorTransaction for MetalDeviceTransaction {
    fn add(
        &mut self,
        connector: &Rc<dyn Connector>,
        change: BackendConnectorState,
    ) -> Result<(), BackendConnectorTransactionError> {
        let Ok(connector) = (connector.clone() as Rc<dyn Any>).downcast::<MetalConnector>() else {
            return Err(BackendConnectorTransactionError::UnsupportedConnectorType(
                connector.kernel_id(),
            ));
        };
        self.add(&connector, change)?;
        Ok(())
    }

    fn prepare(
        self: Box<Self>,
    ) -> Result<Box<dyn BackendPreparedConnectorTransaction>, BackendConnectorTransactionError>
    {
        self.calculate_drm_state()?
            .calculate_change(true, false)
            .map(|v| Box::new(v) as _)
    }
}

impl BackendPreparedConnectorTransaction for MetalDeviceTransactionWithChange {
    fn apply(
        self: Box<Self>,
    ) -> Result<Box<dyn BackendAppliedConnectorTransaction>, BackendConnectorTransactionError> {
        (*self).apply().map(|v| Box::new(v) as _)
    }
}

impl BackendAppliedConnectorTransaction for MetalDeviceAppliedTransaction {
    fn commit(self: Box<Self>) {
        // nothing
    }

    fn rollback(self: Box<Self>) -> Result<(), BackendConnectorTransactionError> {
        (*self).rollback()
    }
}

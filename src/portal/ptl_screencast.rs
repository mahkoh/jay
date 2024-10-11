mod screencast_gui;

use {
    crate::{
        allocator::{AllocatorError, BufferObject, BufferUsage, BO_USE_RENDERING},
        dbus::{prelude::Variant, DbusObject, DictEntry, PendingReply},
        format::{Format, XRGB8888},
        ifs::{jay_compositor::GET_TOPLEVEL_SINCE, jay_screencast::CLIENT_BUFFERS_SINCE},
        pipewire::{
            pw_con::PwCon,
            pw_ifs::pw_client_node::{
                PwClientNode, PwClientNodeBufferConfig, PwClientNodeOwner, PwClientNodePort,
                PwClientNodePortSupportedFormat, PwClientNodePortSupportedFormats,
                SUPPORTED_META_VIDEO_CROP,
            },
            pw_pod::{
                spa_point, spa_rectangle, spa_region, PwPodRectangle, SPA_DATA_DmaBuf,
                SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video, SpaChunkFlags, SPA_STATUS_HAVE_DATA,
                SPA_VIDEO_FORMAT_UNKNOWN,
            },
        },
        portal::{
            ptl_display::{PortalDisplay, PortalDisplayId, PortalOutput},
            ptl_remote_desktop::RemoteDesktopPhase,
            ptl_screencast::screencast_gui::SelectionGui,
            ptl_session::{PortalSession, PortalSessionReply},
            PortalState, PORTAL_SUCCESS,
        },
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            opaque::Opaque,
        },
        video::{dmabuf::DmaBuf, Modifier, LINEAR_MODIFIER},
        wire::jay_screencast::Ready,
        wire_dbus::{
            org,
            org::freedesktop::impl_::portal::{
                screen_cast::{
                    CreateSession, CreateSessionReply, SelectSources, SelectSourcesReply, Start,
                    StartReply,
                },
                session::CloseReply as SessionCloseReply,
            },
        },
        wl_usr::usr_ifs::{
            usr_jay_screencast::{
                UsrJayScreencast, UsrJayScreencastOwner, UsrJayScreencastServerConfig,
            },
            usr_jay_select_toplevel::UsrJaySelectToplevel,
            usr_jay_select_workspace::UsrJaySelectWorkspace,
            usr_jay_toplevel::UsrJayToplevel,
            usr_jay_workspace::UsrJayWorkspace,
            usr_linux_buffer_params::{UsrLinuxBufferParams, UsrLinuxBufferParamsOwner},
            usr_wl_buffer::UsrWlBuffer,
        },
    },
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
        sync::atomic::Ordering::{Acquire, Relaxed, Release},
    },
    thiserror::Error,
};

#[derive(Clone)]
pub enum ScreencastPhase {
    Init,
    SourcesSelected(Rc<SourcesSelectedScreencast>),
    Selecting(Rc<SelectingScreencast>),
    SelectingWindow(Rc<SelectingWindowScreencast>),
    SelectingWorkspace(Rc<SelectingWorkspaceScreencast>),
    Starting(Rc<StartingScreencast>),
    Started(Rc<StartedScreencast>),
    Terminated,
}

unsafe impl UnsafeCellCloneSafe for ScreencastPhase {}

pub struct SourcesSelectedScreencast {
    pub restore_data: Cell<Option<Result<RestoreData, RestoreError>>>,
}

#[derive(Clone)]
pub struct SelectingScreencastCore {
    pub session: Rc<PortalSession>,
    pub request_obj: Rc<DbusObject>,
}

pub struct SelectingScreencast {
    pub core: SelectingScreencastCore,
    pub guis: CopyHashMap<PortalDisplayId, Rc<SelectionGui>>,
    pub restore_data: Cell<Option<RestoreData>>,
}

pub struct SelectingWindowScreencast {
    pub core: SelectingScreencastCore,
    pub dpy: Rc<PortalDisplay>,
    pub selector: Rc<UsrJaySelectToplevel>,
    pub restoring: bool,
}

pub struct SelectingWorkspaceScreencast {
    pub core: SelectingScreencastCore,
    pub dpy: Rc<PortalDisplay>,
    pub selector: Rc<UsrJaySelectWorkspace>,
}

pub struct StartingScreencast {
    pub session: Rc<PortalSession>,
    pub _request_obj: Rc<DbusObject>,
    pub node: Rc<PwClientNode>,
    pub dpy: Rc<PortalDisplay>,
    pub target: ScreencastTarget,
}

pub enum ScreencastTarget {
    Output(Rc<PortalOutput>),
    Workspace(Rc<PortalOutput>, Rc<UsrJayWorkspace>, bool),
    Toplevel(Rc<UsrJayToplevel>),
}

pub struct StartedScreencast {
    pub session: Rc<PortalSession>,
    pub node: Rc<PwClientNode>,
    pub port: Rc<PwClientNodePort>,
    pub buffer_objects: RefCell<Vec<Rc<dyn BufferObject>>>,
    pub buffers: RefCell<Vec<DmaBuf>>,
    pub pending_buffers: RefCell<Vec<Rc<UsrLinuxBufferParams>>>,
    pub buffers_valid: Cell<bool>,
    pub dpy: Rc<PortalDisplay>,
    pub jay_screencast: Rc<UsrJayScreencast>,
    pub port_buffer_valid: Cell<bool>,
    pub fixated: Cell<bool>,
    pub format: Cell<&'static Format>,
    pub modifier: Cell<Modifier>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
}

bitflags! {
    CursorModes: u32;

    HIDDEN = 1,
    EMBEDDED = 2,
    METADATA = 4,
}

bitflags! {
    SourceTypes: u32;

    MONITOR = 1,
    WINDOW = 2,
}

impl PwClientNodeOwner for StartingScreencast {
    fn bound_id(&self, node_id: u32) {
        {
            let output = match &self.target {
                ScreencastTarget::Output(o) => Some(o),
                ScreencastTarget::Workspace(o, _, _) => Some(o),
                ScreencastTarget::Toplevel(_) => None,
            };
            let mapping_id = output.and_then(|o| o.wl.name.borrow().clone());
            self.session.send_start_reply(
                Some(node_id),
                create_restore_data(&self.dpy, &self.target),
                mapping_id.as_deref(),
            );
        }
        let mut supported_formats = PwClientNodePortSupportedFormats {
            media_type: Some(SPA_MEDIA_TYPE_video),
            media_sub_type: Some(SPA_MEDIA_SUBTYPE_raw),
            video_size: None,
            formats: vec![PwClientNodePortSupportedFormat {
                format: XRGB8888,
                modifiers: vec![LINEAR_MODIFIER],
            }],
        };
        if let Some(ctx) = self.dpy.render_ctx.get() {
            if let Some(server_formats) = &ctx.server_formats {
                supported_formats.formats.clear();
                for format in server_formats.values() {
                    if format.write_modifiers.is_empty() {
                        continue;
                    }
                    if format.format.pipewire == SPA_VIDEO_FORMAT_UNKNOWN {
                        continue;
                    }
                    let ptl_format = PwClientNodePortSupportedFormat {
                        format: format.format,
                        modifiers: format.write_modifiers.keys().copied().collect(),
                    };
                    supported_formats.formats.push(ptl_format);
                }
            }
        }
        let jsc_version = self.dpy.jc.version;
        let num_buffers = (jsc_version >= CLIENT_BUFFERS_SINCE).then_some(3);
        let port = self.node.create_port(true, supported_formats, num_buffers);
        port.can_alloc_buffers.set(true);
        port.supported_metas.set(SUPPORTED_META_VIDEO_CROP);
        let jsc = self.dpy.jc.create_screencast();
        match &self.target {
            ScreencastTarget::Output(o) => {
                jsc.set_output(&o.jay);
                jsc.set_allow_all_workspaces(true);
            }
            ScreencastTarget::Workspace(o, ws, _) => {
                jsc.set_output(&o.jay);
                jsc.allow_workspace(ws);
            }
            ScreencastTarget::Toplevel(t) => jsc.set_toplevel(t),
        }
        jsc.set_use_linear_buffers(true);
        jsc.configure();
        match &self.target {
            ScreencastTarget::Output(_) => {}
            ScreencastTarget::Workspace(_, w, true) => {
                self.dpy.con.remove_obj(&**w);
            }
            ScreencastTarget::Workspace(_, _, false) => {}
            ScreencastTarget::Toplevel(t) => {
                self.dpy.con.remove_obj(&**t);
            }
        }
        let started = Rc::new(StartedScreencast {
            session: self.session.clone(),
            node: self.node.clone(),
            port,
            buffer_objects: Default::default(),
            buffers: Default::default(),
            pending_buffers: Default::default(),
            buffers_valid: Cell::new(false),
            dpy: self.dpy.clone(),
            jay_screencast: jsc,
            port_buffer_valid: Cell::new(false),
            fixated: Cell::new(jsc_version < CLIENT_BUFFERS_SINCE),
            format: Cell::new(XRGB8888),
            modifier: Cell::new(LINEAR_MODIFIER),
            width: Cell::new(1),
            height: Cell::new(1),
        });
        self.session
            .sc_phase
            .set(ScreencastPhase::Started(started.clone()));
        started.jay_screencast.owner.set(Some(started.clone()));
        self.node.owner.set(Some(started.clone()));
    }
}

impl PwClientNodeOwner for StartedScreencast {
    fn port_format_changed(&self, port: &Rc<PwClientNodePort>) {
        let format = &*port.negotiated_format.borrow();
        if self.fixated.get() {
            return;
        }
        let (Some(fmt), Some(modifiers)) = (format.format, &format.modifiers) else {
            return;
        };
        let modifier;
        let planes;
        match self.allocate_buffer(fmt, modifiers, 1, 1) {
            Ok(bo) => {
                let dmabuf = bo.dmabuf();
                modifier = dmabuf.modifier;
                planes = dmabuf.planes.len();
            }
            Err(e) => {
                log::error!("Could not allocate buffer: {}", ErrorFmt(e));
                self.session.kill();
                return;
            }
        };
        self.port.supported_formats.borrow_mut().formats = vec![PwClientNodePortSupportedFormat {
            format: fmt,
            modifiers: vec![modifier],
        }];
        self.port.buffer_config.borrow_mut().planes = Some(planes);
        self.node.send_port_update(&self.port, true);
        self.format.set(fmt);
        self.modifier.set(modifier);
        self.fixated.set(true);
    }

    fn use_buffers(self: Rc<Self>, port: &Rc<PwClientNodePort>) {
        if self.jay_screencast.version < CLIENT_BUFFERS_SINCE {
            self.node
                .send_port_output_buffers(port, &self.buffers.borrow_mut());
            self.buffers_valid.set(true);
            return;
        }
        self.buffers_valid.set(false);
        self.port_buffer_valid.set(false);
        let Some(dmabuf) = self.dpy.dmabuf.get() else {
            log::error!("Display does not support dmabuf");
            self.session.kill();
            return;
        };
        self.jay_screencast.clear_buffers();
        self.jay_screencast.configure();
        self.buffer_objects.borrow_mut().clear();
        self.buffers.borrow_mut().clear();
        for buffer in self.pending_buffers.borrow_mut().drain(..) {
            self.dpy.con.remove_obj(&*buffer);
        }
        for _ in 0..self.port.buffers.borrow().len() {
            let res = self.allocate_buffer(
                self.format.get(),
                &[self.modifier.get()],
                self.width.get(),
                self.height.get(),
            );
            match res {
                Ok(b) => {
                    let params = dmabuf.create_params();
                    params.create(&b.dmabuf());
                    params.owner.set(Some(self.clone()));
                    self.buffers.borrow_mut().push(b.dmabuf().clone());
                    self.buffer_objects.borrow_mut().push(b);
                    self.pending_buffers.borrow_mut().push(params);
                }
                Err(e) => {
                    log::error!("Could not allocate buffer: {}", ErrorFmt(e));
                    self.session.kill();
                    return;
                }
            }
        }
        self.node
            .send_port_output_buffers(&self.port, &self.buffers.borrow());
    }

    fn start(self: Rc<Self>) {
        self.jay_screencast.set_running(true);
        self.jay_screencast.configure();
    }

    fn pause(self: Rc<Self>) {
        self.jay_screencast.set_running(false);
        self.jay_screencast.configure();
    }

    fn suspend(self: Rc<Self>) {
        self.jay_screencast.set_running(false);
        self.jay_screencast.configure();
    }
}

#[derive(Debug, Error)]
enum BufferAllocationError {
    #[error("Display has no render context")]
    NoRenderContext,
    #[error(transparent)]
    Allocator(#[from] AllocatorError),
}

impl StartedScreencast {
    fn allocate_buffer(
        &self,
        format: &'static Format,
        modifiers: &[Modifier],
        width: i32,
        height: i32,
    ) -> Result<Rc<dyn BufferObject>, BufferAllocationError> {
        let Some(ctx) = self.dpy.render_ctx.get() else {
            return Err(BufferAllocationError::NoRenderContext);
        };
        let mut usage = BO_USE_RENDERING;
        if let Some(sf) = &ctx.server_formats {
            if let Some(format) = sf.get(&format.drm) {
                let no_render_usage = modifiers.iter().all(|m| {
                    format
                        .write_modifiers
                        .get(m)
                        .map(|w| !w.needs_render_usage)
                        .unwrap_or(false)
                });
                if no_render_usage {
                    usage = BufferUsage::none();
                }
            }
        }
        let buffer = ctx.ctx.ctx.allocator().create_bo(
            &self.dpy.state.dma_buf_ids,
            width,
            height,
            format,
            modifiers,
            usage,
        )?;
        Ok(buffer)
    }
}

impl SelectingScreencastCore {
    pub fn starting(&self, dpy: &Rc<PortalDisplay>, target: ScreencastTarget) {
        let Some(pw_con) = &self.session.pw_con else {
            self.session.kill();
            return;
        };
        let node = pw_con.create_client_node(&[
            ("media.class".to_string(), "Video/Source".to_string()),
            ("node.name".to_string(), "jay-desktop-portal".to_string()),
            ("node.driver".to_string(), "true".to_string()),
        ]);
        let starting = Rc::new(StartingScreencast {
            session: self.session.clone(),
            _request_obj: self.request_obj.clone(),
            node,
            dpy: dpy.clone(),
            target,
        });
        self.session
            .sc_phase
            .set(ScreencastPhase::Starting(starting.clone()));
        starting.node.owner.set(Some(starting.clone()));
        dpy.sessions.set(
            self.session.session_obj.path().to_owned(),
            self.session.clone(),
        );
    }
}

impl PortalSession {
    fn dbus_select_sources(
        self: &Rc<Self>,
        req: SelectSources,
        reply: PendingReply<SelectSourcesReply<'static>>,
    ) {
        match self.sc_phase.get() {
            ScreencastPhase::Init => {}
            _ => {
                self.kill();
                reply.err("Sources have already been selected");
                return;
            }
        }
        self.sc_phase.set(ScreencastPhase::SourcesSelected(Rc::new(
            SourcesSelectedScreencast {
                restore_data: Cell::new(get_restore_data(&req)),
            },
        )));
        reply.ok(&SelectSourcesReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }

    fn dbus_start_screencast(
        self: &Rc<Self>,
        req: Start<'_>,
        reply: PendingReply<StartReply<'static>>,
    ) {
        let restore_data = match self.sc_phase.get() {
            ScreencastPhase::SourcesSelected(s) => s.restore_data.take(),
            _ => {
                self.kill();
                reply.err("Session is not in the correct phase for starting");
                return;
            }
        };
        let request_obj = match self.state.dbus.add_object(req.handle.to_string()) {
            Ok(r) => Rc::new(r),
            Err(_) => {
                self.kill();
                reply.err("Request handle is not unique");
                return;
            }
        };
        {
            use org::freedesktop::impl_::portal::request::*;
            request_obj.add_method::<Close, _>({
                let slf = self.clone();
                move |_, pr| {
                    slf.kill();
                    pr.ok(&CloseReply);
                }
            });
        }
        self.start_reply
            .set(Some(PortalSessionReply::ScreenCast(reply)));
        self.screencast_restore(&request_obj, restore_data, None);
    }

    fn start_interactive_selection(
        self: &Rc<Self>,
        request_obj: &Rc<DbusObject>,
        restore_data: Option<RestoreData>,
    ) {
        let guis = CopyHashMap::new();
        for dpy in self.state.displays.lock().values() {
            if dpy.outputs.len() > 0 {
                guis.set(dpy.id, SelectionGui::new(self, dpy, restore_data.is_some()));
            }
        }
        if guis.is_empty() {
            self.kill();
            self.reply_err("There are no running displays");
            return;
        }
        self.sc_phase
            .set(ScreencastPhase::Selecting(Rc::new(SelectingScreencast {
                core: SelectingScreencastCore {
                    session: self.clone(),
                    request_obj: request_obj.clone(),
                },
                guis,
                restore_data: Cell::new(restore_data),
            })));
    }

    pub fn screencast_restore(
        self: &Rc<Self>,
        request_obj: &Rc<DbusObject>,
        restore_data: Option<Result<RestoreData, RestoreError>>,
        display: Option<Rc<PortalDisplay>>,
    ) {
        if let Some(rd) = restore_data {
            if let Err(e) = self.try_restore(&request_obj, rd, display) {
                log::error!("Could not restore session: {}", ErrorFmt(e));
            } else {
                return;
            }
        }
        self.start_interactive_selection(&request_obj, None);
    }

    fn try_restore(
        self: &Rc<Self>,
        request_obj: &Rc<DbusObject>,
        restore_data: Result<RestoreData, RestoreError>,
        display: Option<Rc<PortalDisplay>>,
    ) -> Result<(), RestoreError> {
        let rd = restore_data?;
        let dpy = if let Some(dpy) = display {
            dpy
        } else {
            let dpy = self
                .state
                .displays
                .lock()
                .values()
                .find(|d| d.unique_id == rd.display)
                .cloned();
            match dpy {
                Some(dpy) => dpy,
                _ => {
                    if self.state.displays.len() == 0 {
                        return Err(RestoreError::UnknownDisplay);
                    } else if self.state.displays.len() == 1 {
                        self.state.displays.lock().values().next().unwrap().clone()
                    } else {
                        self.start_interactive_selection(&request_obj, Some(rd));
                        return Ok(());
                    }
                }
            }
        };
        let start = |target: ScreencastTarget| {
            SelectingScreencastCore {
                session: self.clone(),
                request_obj: request_obj.clone(),
            }
            .starting(&dpy, target);
        };
        match &rd.ty {
            RestoreDataType::Output(d) => {
                let output = dpy
                    .outputs
                    .lock()
                    .values()
                    .find(|o| o.wl.name.borrow().as_ref() == Some(&d.name))
                    .cloned();
                let Some(output) = output else {
                    return Err(RestoreError::UnknownOutput);
                };
                start(ScreencastTarget::Output(output));
            }
            RestoreDataType::Workspace(ws) => {
                let ws = dpy
                    .workspaces
                    .lock()
                    .values()
                    .find(|w| w.name.borrow().as_ref() == Some(&ws.name))
                    .cloned();
                let Some(ws) = ws else {
                    return Err(RestoreError::UnknownWorkspace);
                };
                let Some(output) = dpy.outputs.get(&ws.output.get()) else {
                    return Err(RestoreError::UnknownOutput);
                };
                start(ScreencastTarget::Workspace(output, ws, false));
            }
            RestoreDataType::Toplevel(d) => {
                if dpy.jc.version < GET_TOPLEVEL_SINCE {
                    return Err(RestoreError::GetToplevel);
                }
                let selector = dpy.jc.get_toplevel(&d.id);
                let selecting = Rc::new(SelectingWindowScreencast {
                    core: SelectingScreencastCore {
                        session: self.clone(),
                        request_obj: request_obj.clone(),
                    },
                    dpy: dpy.clone(),
                    selector: selector.clone(),
                    restoring: true,
                });
                selector.owner.set(Some(selecting.clone()));
                self.sc_phase
                    .set(ScreencastPhase::SelectingWindow(selecting));
            }
        }
        Ok(())
    }
}

impl UsrJayScreencastOwner for StartedScreencast {
    fn buffers(&self, buffers: Vec<DmaBuf>) {
        if buffers.len() == 0 {
            return;
        }
        let buffer = &buffers[0];
        *self.port.supported_formats.borrow_mut() = PwClientNodePortSupportedFormats {
            media_type: Some(SPA_MEDIA_TYPE_video),
            media_sub_type: Some(SPA_MEDIA_SUBTYPE_raw),
            video_size: Some(PwPodRectangle {
                width: buffer.width as _,
                height: buffer.height as _,
            }),
            formats: vec![PwClientNodePortSupportedFormat {
                format: buffer.format,
                modifiers: vec![buffer.modifier],
            }],
        };
        *self.port.buffer_config.borrow_mut() = PwClientNodeBufferConfig {
            num_buffers: Some(buffers.len()),
            planes: Some(buffer.planes.len()),
            data_type: SPA_DATA_DmaBuf,
        };
        self.node.send_port_update(&self.port, true);
        self.node.send_active(true);
        self.fixated.set(true);
        *self.buffers.borrow_mut() = buffers;
        self.buffers_valid.set(false);
    }

    fn ready(&self, ev: &Ready) {
        let idx = ev.idx as usize;
        let buffers = &*self.buffers.borrow();
        let pbuffers = self.port.buffers.borrow();
        let buffer = &buffers[idx];
        let discard_buffer = || {
            self.jay_screencast.release_buffer(idx);
        };
        if !self.buffers_valid.get() {
            return;
        }
        let Some(io) = self.port.io_buffers.get() else {
            discard_buffer();
            return;
        };
        let Some(pbuffer) = pbuffers.get(idx) else {
            discard_buffer();
            return;
        };
        let io = unsafe { io.read() };
        if io.status.load(Acquire) == SPA_STATUS_HAVE_DATA.0 {
            discard_buffer();
            return;
        }
        for (chunk, plane) in pbuffer.chunks.iter().zip(buffer.planes.iter()) {
            let chunk = unsafe { chunk.write() };
            chunk.flags = SpaChunkFlags::none();
            chunk.offset = plane.offset;
            chunk.stride = plane.stride;
            chunk.size = plane.stride * buffer.height as u32;
        }
        if let Some(crop) = &pbuffer.meta_video_crop {
            unsafe { crop.write() }.region = spa_region {
                position: spa_point { x: 0, y: 0 },
                size: spa_rectangle {
                    width: buffer.width as _,
                    height: buffer.height as _,
                },
            };
        }
        let buffer_id = io.buffer_id.load(Relaxed) as usize;
        if self.port_buffer_valid.get() {
            if buffer_id != idx {
                if buffer_id < buffers.len() {
                    self.jay_screencast.release_buffer(buffer_id);
                }
            }
        }
        io.buffer_id.store(ev.idx, Relaxed);
        io.status.store(SPA_STATUS_HAVE_DATA.0, Release);
        self.port_buffer_valid.set(true);
        self.port.node.drive();
    }

    fn destroyed(&self) {
        self.session.kill();
    }

    fn config(&self, config: UsrJayScreencastServerConfig) {
        self.width.set(config.width.max(1));
        self.height.set(config.height.max(1));
        self.port.supported_formats.borrow_mut().video_size = Some(PwPodRectangle {
            width: self.width.get() as _,
            height: self.height.get() as _,
        });
        self.node.send_port_update(&self.port, self.fixated.get());
        self.node.send_active(true);
    }
}

impl UsrLinuxBufferParamsOwner for StartedScreencast {
    fn created(&self, buffer: Rc<UsrWlBuffer>) {
        self.buffers_valid.set(true);
        self.jay_screencast.add_buffer(&buffer);
        self.jay_screencast.configure();
        self.dpy.con.remove_obj(&*buffer);
    }

    fn failed(&self) {
        log::error!("Buffer import failed");
        self.session.kill();
    }
}

pub(super) fn add_screencast_dbus_members(
    state_: &Rc<PortalState>,
    pw_con: &Rc<PwCon>,
    object: &DbusObject,
) {
    use org::freedesktop::impl_::portal::screen_cast::*;
    let state = state_.clone();
    let pw_con = pw_con.clone();
    object.add_method::<CreateSession, _>(move |req, pr| {
        dbus_create_session(&state, &pw_con, req, pr);
    });
    let state = state_.clone();
    object.add_method::<SelectSources, _>(move |req, pr| {
        dbus_select_sources(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<Start, _>(move |req, pr| {
        dbus_start(&state, req, pr);
    });
    object.set_property::<AvailableSourceTypes>(Variant::U32(MONITOR.0));
    object.set_property::<AvailableCursorModes>(Variant::U32(EMBEDDED.0));
    object.set_property::<version>(Variant::U32(4));
}

fn dbus_create_session(
    state: &Rc<PortalState>,
    pw_con: &Rc<PwCon>,
    req: CreateSession,
    reply: PendingReply<CreateSessionReply<'static>>,
) {
    log::info!("Create Session {:#?}", req);
    if state.sessions.contains(req.session_handle.0.deref()) {
        reply.err("Session already exists");
        return;
    }
    let obj = match state.dbus.add_object(req.session_handle.0.to_string()) {
        Ok(obj) => obj,
        Err(_) => {
            reply.err("Session path is not unique");
            return;
        }
    };
    let session = Rc::new(PortalSession {
        _id: state.id(),
        state: state.clone(),
        pw_con: Some(pw_con.clone()),
        app: req.app_id.to_string(),
        session_obj: obj,
        sc_phase: CloneCell::new(ScreencastPhase::Init),
        rd_phase: CloneCell::new(RemoteDesktopPhase::Init),
        start_reply: Default::default(),
    });
    {
        use org::freedesktop::impl_::portal::session::*;
        let ses = session.clone();
        session.session_obj.add_method::<Close, _>(move |_, pr| {
            ses.kill();
            pr.ok(&SessionCloseReply);
        });
        session.session_obj.set_property::<version>(Variant::U32(4));
    }
    state
        .sessions
        .set(req.session_handle.0.to_string(), session);
    reply.ok(&CreateSessionReply {
        response: PORTAL_SUCCESS,
        results: Default::default(),
    });
}

fn dbus_select_sources(
    state: &Rc<PortalState>,
    req: SelectSources,
    reply: PendingReply<SelectSourcesReply<'static>>,
) {
    if let Some(s) = get_session(state, &reply, &req.session_handle.0) {
        s.dbus_select_sources(req, reply);
    }
}

fn dbus_start(state: &Rc<PortalState>, req: Start, reply: PendingReply<StartReply<'static>>) {
    if let Some(s) = get_session(state, &reply, &req.session_handle.0) {
        s.dbus_start_screencast(req, reply);
    }
}

fn get_session<T>(
    state: &Rc<PortalState>,
    reply: &PendingReply<T>,
    handle: &str,
) -> Option<Rc<PortalSession>> {
    let res = state.sessions.get(handle);
    if res.is_none() {
        let msg = format!("Screencast session `{}` does not exist", handle);
        reply.err(&msg);
    }
    res
}

fn create_restore_data(dpy: &PortalDisplay, rd: &ScreencastTarget) -> Option<Variant<'static>> {
    let rd = RestoreData {
        display: dpy.unique_id,
        ty: match rd {
            ScreencastTarget::Output(o) => RestoreDataType::Output(RestoreDataOutput {
                name: o.wl.name.borrow().clone()?,
            }),
            ScreencastTarget::Workspace(_, w, _) => {
                RestoreDataType::Workspace(RestoreDataWorkspace {
                    name: w.name.borrow().clone()?,
                })
            }
            ScreencastTarget::Toplevel(tl) => RestoreDataType::Toplevel(RestoreDataToplevel {
                id: tl.toplevel_id.borrow().clone()?,
            }),
        },
    };
    Some(Variant::Struct(vec![
        Variant::String("Jay".into()),
        Variant::U32(1),
        Variant::Variant(Box::new(Variant::String(
            serde_json::to_string(&rd).unwrap().into(),
        ))),
    ]))
}

#[derive(Debug, Error)]
pub enum RestoreError {
    #[error("DBus restore data is not a struct")]
    NotAStruct,
    #[error("DBus restore data is not a struct with 3 fields")]
    NotLen3,
    #[error("DBus restore data first field is not a string")]
    FirstNotString,
    #[error("DBus restore data second field is not a u32")]
    SecondNotU32,
    #[error("DBus restore data third field is not a variant")]
    ThirdNotVariant,
    #[error("DBus restore data third field is not a string")]
    ThirdNotString,
    #[error("DBus restore data is not for Jay")]
    NotJay,
    #[error("DBus restore data is not version 1")]
    NotVersion1,
    #[error("DBus restore data could not be deserialized")]
    Parse(#[source] serde_json::Error),
    #[error("The display no longer exists")]
    UnknownDisplay,
    #[error("The output no longer exists")]
    UnknownOutput,
    #[error("The workspace no longer exists")]
    UnknownWorkspace,
    #[error("The display does not support toplevel restoration")]
    GetToplevel,
}

fn get_restore_data(req: &SelectSources) -> Option<Result<RestoreData, RestoreError>> {
    let restore_data = req.options.iter().find(|n| n.key == "restore_data")?;
    Some(get_restore_data_(restore_data))
}

fn get_restore_data_(
    restore_data: &DictEntry<Cow<str>, Variant>,
) -> Result<RestoreData, RestoreError> {
    let Variant::Struct(s) = &restore_data.value else {
        return Err(RestoreError::NotAStruct);
    };
    if s.len() != 3 {
        return Err(RestoreError::NotLen3);
    }
    let Variant::String(compositor) = &s[0] else {
        return Err(RestoreError::FirstNotString);
    };
    let Variant::U32(version) = &s[1] else {
        return Err(RestoreError::SecondNotU32);
    };
    let Variant::Variant(restore_data) = &s[2] else {
        return Err(RestoreError::ThirdNotVariant);
    };
    let Variant::String(restore_data) = &**restore_data else {
        return Err(RestoreError::ThirdNotString);
    };
    if compositor != "Jay" {
        return Err(RestoreError::NotJay);
    }
    if *version != 1 {
        return Err(RestoreError::NotVersion1);
    }
    serde_json::from_str(restore_data).map_err(RestoreError::Parse)
}

#[derive(Serialize, Deserialize)]
pub struct RestoreData {
    display: Opaque,
    ty: RestoreDataType,
}

#[derive(Serialize, Deserialize)]
enum RestoreDataType {
    Output(RestoreDataOutput),
    Workspace(RestoreDataWorkspace),
    Toplevel(RestoreDataToplevel),
}

#[derive(Serialize, Deserialize)]
struct RestoreDataOutput {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct RestoreDataWorkspace {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct RestoreDataToplevel {
    id: String,
}

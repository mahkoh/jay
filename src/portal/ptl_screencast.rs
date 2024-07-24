mod screencast_gui;

use {
    crate::{
        dbus::{prelude::Variant, DbusObject, DictEntry, DynamicType, PendingReply},
        pipewire::{
            pw_con::PwCon,
            pw_ifs::pw_client_node::{
                PwClientNode, PwClientNodeBufferConfig, PwClientNodeOwner, PwClientNodePort,
                PwClientNodePortSupportedFormats, SUPPORTED_META_VIDEO_CROP,
            },
            pw_pod::{
                spa_point, spa_rectangle, spa_region, PwPodRectangle, SPA_DATA_DmaBuf,
                SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video, SpaChunkFlags, SPA_STATUS_HAVE_DATA,
            },
        },
        portal::{
            ptl_display::{PortalDisplay, PortalDisplayId, PortalOutput},
            ptl_screencast::screencast_gui::SelectionGui,
            PortalState, PORTAL_SUCCESS,
        },
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
        },
        video::dmabuf::{DmaBuf, PlaneVec},
        wire::jay_screencast::Ready,
        wire_dbus::{
            org,
            org::freedesktop::impl_::portal::{
                screen_cast::{
                    CreateSession, CreateSessionReply, SelectSources, SelectSourcesReply, Start,
                    StartReply,
                },
                session::{CloseReply as SessionCloseReply, Closed},
            },
        },
        wl_usr::usr_ifs::{
            usr_jay_screencast::{UsrJayScreencast, UsrJayScreencastOwner},
            usr_jay_select_toplevel::UsrJaySelectToplevel,
            usr_jay_select_workspace::UsrJaySelectWorkspace,
            usr_jay_toplevel::UsrJayToplevel,
            usr_jay_workspace::UsrJayWorkspace,
        },
    },
    std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
        sync::atomic::Ordering::{Acquire, Relaxed, Release},
    },
};

shared_ids!(ScreencastSessionId);
pub struct ScreencastSession {
    _id: ScreencastSessionId,
    state: Rc<PortalState>,
    pw_con: Rc<PwCon>,
    pub app: String,
    session_obj: DbusObject,
    pub phase: CloneCell<ScreencastPhase>,
}

#[derive(Clone)]
pub enum ScreencastPhase {
    Init,
    SourcesSelected,
    Selecting(Rc<SelectingScreencast>),
    SelectingWindow(Rc<SelectingWindowScreencast>),
    SelectingWorkspace(Rc<SelectingWorkspaceScreencast>),
    Starting(Rc<StartingScreencast>),
    Started(Rc<StartedScreencast>),
    Terminated,
}

unsafe impl UnsafeCellCloneSafe for ScreencastPhase {}

#[derive(Clone)]
pub struct SelectingScreencastCore {
    pub session: Rc<ScreencastSession>,
    pub request_obj: Rc<DbusObject>,
    pub reply: Rc<PendingReply<StartReply<'static>>>,
}

pub struct SelectingScreencast {
    pub core: SelectingScreencastCore,
    pub guis: CopyHashMap<PortalDisplayId, Rc<SelectionGui>>,
}

pub struct SelectingWindowScreencast {
    pub core: SelectingScreencastCore,
    pub dpy: Rc<PortalDisplay>,
    pub selector: Rc<UsrJaySelectToplevel>,
}

pub struct SelectingWorkspaceScreencast {
    pub core: SelectingScreencastCore,
    pub dpy: Rc<PortalDisplay>,
    pub selector: Rc<UsrJaySelectWorkspace>,
}

pub struct StartingScreencast {
    pub session: Rc<ScreencastSession>,
    pub _request_obj: Rc<DbusObject>,
    pub reply: Rc<PendingReply<StartReply<'static>>>,
    pub node: Rc<PwClientNode>,
    pub dpy: Rc<PortalDisplay>,
    pub target: ScreencastTarget,
}

pub enum ScreencastTarget {
    Output(Rc<PortalOutput>),
    Workspace(Rc<PortalOutput>, Rc<UsrJayWorkspace>),
    Toplevel(Rc<UsrJayToplevel>),
}

pub struct StartedScreencast {
    session: Rc<ScreencastSession>,
    node: Rc<PwClientNode>,
    port: Rc<PwClientNodePort>,
    buffers: RefCell<PlaneVec<DmaBuf>>,
    buffers_valid: Cell<bool>,
    dpy: Rc<PortalDisplay>,
    jay_screencast: Rc<UsrJayScreencast>,
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
            let inner_type = DynamicType::DictEntry(
                Box::new(DynamicType::String),
                Box::new(DynamicType::Variant),
            );
            let kt = DynamicType::Struct(vec![
                DynamicType::U32,
                DynamicType::Array(Box::new(inner_type.clone())),
            ]);
            let variants = &[DictEntry {
                key: "streams".into(),
                value: Variant::Array(
                    kt,
                    vec![Variant::U32(node_id), Variant::Array(inner_type, vec![])],
                ),
            }];
            self.reply.ok(&StartReply {
                response: PORTAL_SUCCESS,
                results: Cow::Borrowed(variants),
            });
        }
        let port = self.node.create_port(true);
        port.can_alloc_buffers.set(true);
        port.supported_metas.set(SUPPORTED_META_VIDEO_CROP);
        let jsc = self.dpy.jc.create_screencast();
        match &self.target {
            ScreencastTarget::Output(o) => {
                jsc.set_output(&o.jay);
                jsc.set_allow_all_workspaces(true);
            }
            ScreencastTarget::Workspace(o, ws) => {
                jsc.set_output(&o.jay);
                jsc.allow_workspace(ws);
            }
            ScreencastTarget::Toplevel(t) => jsc.set_toplevel(t),
        }
        jsc.set_use_linear_buffers(true);
        jsc.configure();
        match &self.target {
            ScreencastTarget::Output(_) => {}
            ScreencastTarget::Workspace(_, w) => {
                self.dpy.con.remove_obj(&**w);
            }
            ScreencastTarget::Toplevel(t) => {
                self.dpy.con.remove_obj(&**t);
            }
        }
        let started = Rc::new(StartedScreencast {
            session: self.session.clone(),
            node: self.node.clone(),
            port,
            buffers: Default::default(),
            buffers_valid: Cell::new(false),
            dpy: self.dpy.clone(),
            jay_screencast: jsc,
        });
        self.session
            .phase
            .set(ScreencastPhase::Started(started.clone()));
        started.jay_screencast.owner.set(Some(started.clone()));
        self.node.owner.set(Some(started.clone()));
    }
}

impl PwClientNodeOwner for StartedScreencast {
    fn port_format_changed(&self, port: &Rc<PwClientNodePort>) {
        self.node.send_port_update(port, false);
    }

    fn use_buffers(&self, port: &Rc<PwClientNodePort>) {
        self.node
            .send_port_output_buffers(port, &self.buffers.borrow_mut());
        self.buffers_valid.set(true);
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

impl SelectingScreencastCore {
    pub fn starting(&self, dpy: &Rc<PortalDisplay>, target: ScreencastTarget) {
        let node = self.session.pw_con.create_client_node(&[
            ("media.class".to_string(), "Video/Source".to_string()),
            ("node.name".to_string(), "jay-desktop-portal".to_string()),
            ("node.driver".to_string(), "true".to_string()),
        ]);
        let starting = Rc::new(StartingScreencast {
            session: self.session.clone(),
            _request_obj: self.request_obj.clone(),
            reply: self.reply.clone(),
            node,
            dpy: dpy.clone(),
            target,
        });
        self.session
            .phase
            .set(ScreencastPhase::Starting(starting.clone()));
        starting.node.owner.set(Some(starting.clone()));
        dpy.screencasts.set(
            self.session.session_obj.path().to_owned(),
            self.session.clone(),
        );
    }
}

impl ScreencastSession {
    pub(super) fn kill(&self) {
        self.session_obj.emit_signal(&Closed);
        self.state.screencasts.remove(self.session_obj.path());
        match self.phase.set(ScreencastPhase::Terminated) {
            ScreencastPhase::Init => {}
            ScreencastPhase::SourcesSelected => {}
            ScreencastPhase::Terminated => {}
            ScreencastPhase::Selecting(s) => {
                s.core.reply.err("Session has been terminated");
                for gui in s.guis.lock().drain_values() {
                    gui.kill(false);
                }
            }
            ScreencastPhase::SelectingWindow(s) => {
                s.dpy.con.remove_obj(&*s.selector);
                s.core.reply.err("Session has been terminated");
            }
            ScreencastPhase::SelectingWorkspace(s) => {
                s.dpy.con.remove_obj(&*s.selector);
                s.core.reply.err("Session has been terminated");
            }
            ScreencastPhase::Starting(s) => {
                s.reply.err("Session has been terminated");
                s.node.con.destroy_obj(s.node.deref());
                s.dpy.screencasts.remove(self.session_obj.path());
                match &s.target {
                    ScreencastTarget::Output(_) => {}
                    ScreencastTarget::Workspace(_, w) => {
                        s.dpy.con.remove_obj(&**w);
                    }
                    ScreencastTarget::Toplevel(t) => {
                        s.dpy.con.remove_obj(&**t);
                    }
                }
            }
            ScreencastPhase::Started(s) => {
                s.jay_screencast.con.remove_obj(s.jay_screencast.deref());
                s.node.con.destroy_obj(s.node.deref());
                s.dpy.screencasts.remove(self.session_obj.path());
            }
        }
    }

    fn dbus_select_sources(
        self: &Rc<Self>,
        _req: SelectSources,
        reply: PendingReply<SelectSourcesReply<'static>>,
    ) {
        match self.phase.get() {
            ScreencastPhase::Init => {}
            _ => {
                self.kill();
                reply.err("Sources have already been selected");
                return;
            }
        }
        self.phase.set(ScreencastPhase::SourcesSelected);
        reply.ok(&SelectSourcesReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }

    fn dbus_start(self: &Rc<Self>, req: Start<'_>, reply: PendingReply<StartReply<'static>>) {
        match self.phase.get() {
            ScreencastPhase::SourcesSelected => {}
            _ => {
                self.kill();
                reply.err("Session is not in the correct phase for starting");
                return;
            }
        }
        let request_obj = match self.state.dbus.add_object(req.handle.to_string()) {
            Ok(r) => r,
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
        let guis = CopyHashMap::new();
        for dpy in self.state.displays.lock().values() {
            if dpy.outputs.len() > 0 {
                guis.set(dpy.id, SelectionGui::new(self, dpy));
            }
        }
        if guis.is_empty() {
            self.kill();
            reply.err("There are no running displays");
            return;
        }
        self.phase
            .set(ScreencastPhase::Selecting(Rc::new(SelectingScreencast {
                core: SelectingScreencastCore {
                    session: self.clone(),
                    request_obj: Rc::new(request_obj),
                    reply: Rc::new(reply),
                },
                guis,
            })));
    }
}

impl UsrJayScreencastOwner for StartedScreencast {
    fn buffers(&self, buffers: PlaneVec<DmaBuf>) {
        if buffers.len() == 0 {
            return;
        }
        let buffer = &buffers[0];
        *self.port.supported_formats.borrow_mut() = Some(PwClientNodePortSupportedFormats {
            media_type: Some(SPA_MEDIA_TYPE_video),
            media_sub_type: Some(SPA_MEDIA_SUBTYPE_raw),
            video_size: Some(PwPodRectangle {
                width: buffer.width as _,
                height: buffer.height as _,
            }),
            formats: vec![buffer.format],
            modifiers: vec![buffer.modifier],
        });
        let bc = PwClientNodeBufferConfig {
            num_buffers: buffers.len(),
            planes: buffer.planes.len(),
            _stride: Some(buffer.planes[0].stride),
            _size: Some(buffer.planes[0].stride * buffer.height as u32),
            _align: 16,
            data_type: SPA_DATA_DmaBuf,
        };
        self.port.buffer_config.set(Some(bc));
        self.node.send_port_update(&self.port, true);
        self.node.send_active(true);
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
            discard_buffer();
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
        if buffer_id != idx {
            if buffer_id < buffers.len() {
                self.jay_screencast.release_buffer(buffer_id);
            }
        }
        io.buffer_id.store(ev.idx, Relaxed);
        io.status.store(SPA_STATUS_HAVE_DATA.0, Release);
        if let Some(wfd) = self.port.node.transport_out.get() {
            let _ = uapi::eventfd_write(wfd.raw(), 1);
        }
    }

    fn destroyed(&self) {
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
    if state.screencasts.contains(req.session_handle.0.deref()) {
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
    let session = Rc::new(ScreencastSession {
        _id: state.id(),
        state: state.clone(),
        pw_con: pw_con.clone(),
        app: req.app_id.to_string(),
        session_obj: obj,
        phase: CloneCell::new(ScreencastPhase::Init),
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
        .screencasts
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
        s.dbus_start(req, reply);
    }
}

fn get_session<T>(
    state: &Rc<PortalState>,
    reply: &PendingReply<T>,
    handle: &str,
) -> Option<Rc<ScreencastSession>> {
    let res = state.screencasts.get(handle);
    if res.is_none() {
        let msg = format!("Screencast session `{}` does not exist", handle);
        reply.err(&msg);
    }
    res
}

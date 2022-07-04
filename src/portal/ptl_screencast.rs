use {
    crate::{
        dbus::{prelude::Variant, DbusObject, DictEntry, DynamicType, PendingReply},
        pipewire::{
            pw_ifs::pw_client_node::{
                PwClientNode, PwClientNodeBufferConfig, PwClientNodeOwner, PwClientNodePort,
                PwClientNodePortSupportedFormats, SUPPORTED_META_BUSY,
            },
            pw_pod::{
                PwPodRectangle, SPA_DATA_DmaBuf, SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video,
                SpaChunkFlags, SPA_STATUS_HAVE_DATA, SPA_STATUS_NEED_DATA,
            },
        },
        portal::{ptl_display::PortalDisplay, PortalState, PORTAL_ENDED, PORTAL_SUCCESS},
        utils::clonecell::{CloneCell, UnsafeCellCloneSafe},
        video::dmabuf::DmaBuf,
        wire::jay_screencast::Ready,
        wire_dbus::{
            org,
            org::freedesktop::impl_::portal::{
                screen_cast::{
                    CreateSession, CreateSessionReply, SelectSources, SelectSourcesReply, Start,
                    StartReply,
                },
                session::{CloseReply as SessionCloseReply, Closed as SessionClosed},
            },
        },
        wl_usr::usr_ifs::usr_jay_screencast::{UsrJayScreencast, UsrJayScreencastOwner},
    },
    std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};
use crate::portal::ptl_selection_gui::SelectionGui;

pub struct ScreencastSession {
    id: u32,
    state: Rc<PortalState>,
    app: String,
    session_obj: DbusObject,
    selection: CloneCell<Option<Rc<SelectedScreencast>>>,
    node: Rc<PwClientNode>,
    port: Rc<PwClientNodePort>,
    pending_start: Cell<Option<PendingReply<StartReply<'static>>>>,
    buffers: RefCell<Vec<DmaBuf>>,
}

#[derive(Clone)]
pub enum ScreencastPhase {
    Init,
    Selecting(Rc<SelectingScreencast>),
    Selected(Rc<SelectedScreencast>),
    Running(Rc<SelectedScreencast>),
}

unsafe impl UnsafeCellCloneSafe for ScreencastPhase {}

pub struct SelectingScreencast {
    session: Rc<ScreencastSession>,
    dpy: Rc<PortalDisplay>,
    sc: Rc<UsrJayScreencast>,
}

pub struct SelectedScreencast {
    session: Rc<ScreencastSession>,
    dpy: Rc<PortalDisplay>,
    sc: Rc<UsrJayScreencast>,
}

impl PwClientNodeOwner for ScreencastSession {
    fn port_format_changed(&self, port: &Rc<PwClientNodePort>) {
        self.node.send_port_update(port);
    }

    fn use_buffers(&self, port: &Rc<PwClientNodePort>) {
        self.node
            .send_port_output_buffers(port, &self.buffers.borrow_mut());
    }

    fn bound_id(&self, node_id: u32) {
        if let Some(start) = self.pending_start.take() {
            self.answer_start(node_id, start);
        }
    }

    fn start(self: Rc<Self>) {
        if let Some(selection) = self.selection.get() {
            selection.sc.request_set_running(true);
            selection.sc.request_configure();
        }
    }
}

impl ScreencastSession {
    fn answer_start(&self, node_id: u32, start: PendingReply<StartReply<'static>>) {
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
        start.ok(&StartReply {
            response: PORTAL_SUCCESS,
            results: Cow::Borrowed(variants),
        });
    }

    fn kill(&self) {}

    fn dbus_select_sources(
        self: &Rc<Self>,
        _req: SelectSources,
        reply: PendingReply<SelectSourcesReply<'static>>,
    ) {
        if self.selection.get().is_some() {
            self.kill();
            reply.err("Sources have already been selected");
            return;
        }
        let dpy = self.state.displays.lock().values().next().cloned();
        let dpy = match dpy {
            Some(dpy) => dpy,
            _ => {
                self.kill();
                reply.err("System has no running displays");
                return;
            }
        };
        SelectionGui::build(&dpy, |_| { }).unwrap();
        let output = dpy.outputs.lock().values().next().map(|o| o.jay.clone());
        let output = match output {
            Some(output) => output,
            _ => {
                self.kill();
                reply.err("Display has no outputs attached");
                return;
            }
        };
        let jsc = Rc::new(UsrJayScreencast {
            id: output.con.id(),
            con: output.con.clone(),
            owner: Default::default(),
            pending_buffers: Default::default(),
            pending_planes: Default::default(),
            pending_config: Default::default(),
        });
        output.con.add_object(jsc.clone());
        dpy.jc.request_create_screencast(&jsc);
        jsc.request_set_output(&output);
        jsc.request_set_use_linear_buffers(true);
        jsc.request_set_allow_all_workspaces(true);
        jsc.request_configure();
        let selected = Rc::new(SelectedScreencast {
            session: self.clone(),
            dpy,
            sc: jsc.clone(),
        });
        selected.dpy.screencasts.set(self.id, selected.clone());
        jsc.owner.set(Some(selected.clone()));
        self.selection.set(Some(selected));
        reply.ok(&SelectSourcesReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }

    fn dbus_start(&self, _req: Start, reply: PendingReply<StartReply<'static>>) {
        if let Some(node_id) = self.node.data.bound_id.get() {
            self.answer_start(node_id, reply);
        } else {
            self.pending_start.set(Some(reply));
        }
    }
}

impl SelectedScreencast {
    fn destroy(&self) {
        self.sc.con.remove_obj(self.sc.deref());
    }
}

impl UsrJayScreencastOwner for SelectedScreencast {
    fn buffers(&self, buffers: Vec<DmaBuf>) {
        if buffers.len() == 0 {
            return;
        }
        let buffer = &buffers[0];
        *self.session.port.supported_formats.borrow_mut() =
            Some(PwClientNodePortSupportedFormats {
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
            stride: Some(buffer.planes[0].stride),
            size: Some(buffer.planes[0].stride * buffer.height as u32),
            align: 16,
            data_type: SPA_DATA_DmaBuf,
        };
        self.session.port.buffer_config.set(Some(bc));
        self.session.node.send_port_update(&self.session.port);
        self.session.node.send_active(true);
        *self.session.buffers.borrow_mut() = buffers;
    }

    fn ready(&self, ev: &Ready) {
        let idx = ev.idx as usize;
        let port = &self.session.port;
        unsafe {
            let mut used = false;
            if let Some(io) = port.io_buffers.lock().values().next() {
                let io = io.write();
                if io.status != SPA_STATUS_HAVE_DATA {
                    used = true;
                    if io.buffer_id != ev.idx {
                        if (io.buffer_id as usize) < self.session.buffers.borrow_mut().len() {
                            self.sc.request_release_buffer(io.buffer_id as usize);
                        }
                    }
                    io.buffer_id = ev.idx;
                    io.status = SPA_STATUS_HAVE_DATA;
                }
            }
            if !used {
                self.sc.request_release_buffer(idx);
            }
            {
                let pbuffers = port.buffers.borrow_mut();
                let buffers = self.session.buffers.borrow_mut();
                if let Some(pbuffer) = pbuffers.get(idx) {
                    let buffer = &buffers[idx];
                    for (chunk, plane) in pbuffer.chunks.iter().zip(buffer.planes.iter()) {
                        let chunk = chunk.write();
                        chunk.flags = SpaChunkFlags::none();
                        chunk.offset = plane.offset;
                        chunk.stride = plane.stride;
                        chunk.size = plane.stride * buffer.height as u32;
                    }
                }
            }
        }
        if let Some(wfd) = port.node.transport_out.get() {
            let _ = uapi::eventfd_write(wfd.raw(), 1);
        }
    }

    fn destroyed(&self) {
        self.session.kill();
    }
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

pub(super) fn add_screencast_dbus_members(state_: &Rc<PortalState>, object: &DbusObject) {
    use org::freedesktop::impl_::portal::screen_cast::*;
    let state = state_.clone();
    object.add_method::<CreateSession, _>(move |req, pr| {
        dbus_create_session(&state, req, pr);
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
    object.set_property::<AvailableCursorModes>(Variant::U32((HIDDEN | EMBEDDED | METADATA).0));
    object.set_property::<version>(Variant::U32(4));
}

fn dbus_create_session(
    state: &Rc<PortalState>,
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
    let node = state.pw_con.create_client_node(&[
        ("media.class".to_string(), "Video/Source".to_string()),
        ("node.name".to_string(), "jay-desktop-portal".to_string()),
        ("node.driver".to_string(), "true".to_string()),
    ]);
    let port = node.create_port(true);
    port.can_alloc_buffers.set(true);
    port.supported_metas.set(SUPPORTED_META_BUSY);
    let session = Rc::new(ScreencastSession {
        id: state.id(),
        state: state.clone(),
        app: req.app_id.to_string(),
        session_obj: obj,
        selection: Default::default(),
        node,
        port,
        pending_start: Cell::new(None),
        buffers: RefCell::new(vec![]),
    });
    session.node.owner.set(Some(session.clone()));
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
    match state.screencasts.get(req.session_handle.0.deref()) {
        Some(s) => s.dbus_select_sources(req, reply),
        _ => {
            let msg = format!(
                "Screencast session `{}` does not exist",
                req.session_handle.0
            );
            reply.err(&msg);
        }
    }
}

fn dbus_start(state: &Rc<PortalState>, req: Start, reply: PendingReply<StartReply<'static>>) {
    match state.screencasts.get(req.session_handle.0.deref()) {
        Some(s) => s.dbus_start(req, reply),
        _ => {
            let msg = format!("Screencast session {} does not exist", req.session_handle.0);
            reply.err(&msg);
        }
    }
}

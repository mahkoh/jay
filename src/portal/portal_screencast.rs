use std::borrow::Cow;
use std::cell::RefCell;
use {
    crate::{
        dbus::{prelude::Variant, DbusObject, DictEntry, DynamicType, PendingReply},
        pipewire::pw_ifs::pw_client_node::{PwClientNode, PwClientNodeOwner},
        portal::{PortalState, PORTAL_ENDED, PORTAL_SUCCESS},
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap},
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
    std::{cell::Cell, ops::Deref, rc::Rc},
};
use crate::pipewire::pw_ifs::pw_client_node::{PwClientNodeBufferConfig, PwClientNodePort, PwClientNodePortSupportedFormats, SUPPORTED_META_BUSY};
use crate::pipewire::pw_pod::{PwPodRectangle, SPA_DATA_DmaBuf, SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video, SPA_STATUS_HAVE_DATA, SPA_STATUS_NEED_DATA, SpaChunkFlags};
use crate::portal::portal_display::PortalDisplay;

#[derive(Default)]
pub struct PortalScreencastsState {
    sessions: CopyHashMap<String, Rc<ScreencastSession>>,
}

struct ScreencastSession {
    state: Rc<PortalState>,
    path: String,
    app: String,
    obj: DbusObject,
    selection: CloneCell<Option<Rc<SelectedScreencast>>>,
    node: Rc<PwClientNode>,
    port: Rc<PwClientNodePort>,
    pending_start: Cell<Option<PendingReply<StartReply<'static>>>>,
    buffers: RefCell<Vec<DmaBuf>>,
}

struct SelectedScreencast {
    session: Rc<ScreencastSession>,
    dpy: Rc<PortalDisplay>,
    sc: Rc<UsrJayScreencast>,
}

impl PwClientNodeOwner for ScreencastSession {
    fn port_format_changed(&self, port: &Rc<PwClientNodePort>) {
        self.node.send_port_update(port);
    }

    fn use_buffers(&self, port: &Rc<PwClientNodePort>) {
        self.node.send_port_output_buffers(port, &self.buffers.borrow_mut());
    }

    fn bound_id(&self, node_id: u32) {
        if let Some(start) = self.pending_start.take() {
            self.answer_start(node_id, start);
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
        *self.session.port.supported_formats.borrow_mut() = Some(PwClientNodePortSupportedFormats {
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
        // {
        //     let buffers = self.buffers.borrow_mut();
        //     let buffer = &buffers[idx];
        //     log::info!("{:?}", buffer);
        //     let ctx = self.dpy.render_ctx.get().unwrap();
        //     let qoi = video_buf_to_qoi(&ctx.ctx.gbm, buffer);
        //     std::fs::write("image.qoi", &qoi[..]).unwrap();
        // }
        let port = &self.session.port;
        unsafe {
            let mut used = false;
            for io in port.io_buffers.lock().values() {
                let io = io.write();
                if io.status != SPA_STATUS_NEED_DATA {
                    log::info!("status = {:?}", io.status);
                    continue;
                }
                used = true;
                if (io.buffer_id as usize) < self.session.buffers.borrow_mut().len() {
                    self.sc.request_release_buffer(io.buffer_id as usize);
                }
                io.buffer_id = idx as _;
                io.status = SPA_STATUS_HAVE_DATA;
            }
            if !used {
                self.sc.request_release_buffer(idx);
            }
            {
                let mut pbuffers = port.buffers.borrow_mut();
                let mut buffers = self.session.buffers.borrow_mut();
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
        log::info!("Screencast was terminated by the compositor");
        self.session
            .state
            .screencasts
            .sessions
            .remove(&self.session.path);
        self.session.obj.emit_signal(&SessionClosed);
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
        create_session(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<SelectSources, _>(move |req, pr| {
        select_sources(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<Start, _>(move |req, pr| {
        start(&state, req, pr);
    });
    object.set_property::<AvailableSourceTypes>(Variant::U32(MONITOR.0));
    object.set_property::<AvailableCursorModes>(Variant::U32((HIDDEN | EMBEDDED | METADATA).0));
    object.set_property::<version>(Variant::U32(4));
}

fn create_session(
    state: &Rc<PortalState>,
    req: CreateSession,
    reply: Option<PendingReply<CreateSessionReply<'static>>>,
) {
    log::info!("Create Session {:#?}", req);
    if state
        .screencasts
        .sessions
        .contains(req.session_handle.0.deref())
    {
        if let Some(reply) = reply {
            reply.err("Session already exists");
        }
        return;
    }
    let obj = match state.dbus.add_object(req.session_handle.0.to_string()) {
        Ok(obj) => obj,
        Err(_) => {
            if let Some(reply) = reply {
                reply.err("Session path is not unique");
            }
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
        state: state.clone(),
        path: req.session_handle.0.to_string(),
        app: req.app_id.to_string(),
        obj,
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
        session.obj.add_method::<Close, _>(move |_, pr| {
            close_session(&ses, pr);
        });
        session.obj.set_property::<version>(Variant::U32(4));
    }
    state
        .screencasts
        .sessions
        .set(req.session_handle.0.to_string(), session);
    if let Some(reply) = reply {
        reply.ok(&CreateSessionReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }
}

fn close_session(session: &Rc<ScreencastSession>, reply: Option<PendingReply<SessionCloseReply>>) {
    log::info!("Close Session {}", session.path);
    if let Some(session) = session.state.screencasts.sessions.remove(&session.path) {
        if let Some(selection) = session.selection.take() {
            selection.destroy();
        }
    }
    if let Some(reply) = reply {
        reply.ok(&SessionCloseReply);
    }
}

fn select_sources(
    state: &Rc<PortalState>,
    req: SelectSources,
    reply: Option<PendingReply<SelectSourcesReply<'static>>>,
) {
    log::info!("{:#?}", req);
    macro_rules! exit {
        ($response:expr) => {{
            if let Some(reply) = reply {
                reply.ok(&SelectSourcesReply {
                    response: $response,
                    results: Default::default(),
                });
            }
            return;
        }};
    }
    let session = match state.screencasts.sessions.get(req.session_handle.0.deref()) {
        Some(s) => s,
        _ => {
            log::error!("Screencast session {} does not exist", req.session_handle.0);
            exit!(PORTAL_ENDED);
        }
    };
    if let Some(selection) = session.selection.take() {
        selection.destroy();
    }
    let dpy = state.displays.lock().values().next().cloned();
    let dpy = match dpy {
        Some(dpy) => dpy,
        _ => exit!(PORTAL_ENDED),
    };
    let output = dpy.outputs.lock().values().next().map(|o| o.jay.clone());
    let output = match output {
        Some(output) => output,
        _ => exit!(PORTAL_ENDED),
    };
    let jsc = Rc::new(UsrJayScreencast {
        id: output.con.id(),
        con: output.con.clone(),
        owner: Default::default(),
        pending_buffers: Default::default(),
        pending_planes: Default::default(),
    });
    output.con.add_object(jsc.clone());
    dpy.jc.request_create_screencast(&jsc);
    jsc.request_set_output(&output);
    jsc.request_set_show_always();
    let selected = Rc::new(SelectedScreencast {
        session: session.clone(),
        dpy,
        sc: jsc.clone(),
    });
    jsc.owner.set(Some(selected.clone()));
    session.selection.set(Some(selected));
    exit!(PORTAL_SUCCESS);
}

fn start(state: &Rc<PortalState>, req: Start, reply: Option<PendingReply<StartReply<'static>>>) {
    log::info!("{:#?}", req);
    macro_rules! exit {
        ($response:expr) => {{
            if let Some(reply) = reply {
                reply.ok(&StartReply {
                    response: $response,
                    results: Default::default(),
                });
            }
            return;
        }};
    }
    let session = match state.screencasts.sessions.get(req.session_handle.0.deref()) {
        Some(s) => s,
        _ => {
            log::error!("Screencast session {} does not exist", req.session_handle.0);
            exit!(PORTAL_ENDED);
        }
    };
    let selection = match session.selection.get() {
        Some(s) => s,
        _ => {
            log::error!("Screencast session has no selection");
            exit!(PORTAL_ENDED);
        }
    };
    selection.sc.request_start();
    if let Some(node_id) = session.node.data.bound_id.get() {
        if let Some(reply) = reply {
            session.answer_start(node_id, reply);
        }
    } else {
        session.pending_start.set(reply);
    }
}

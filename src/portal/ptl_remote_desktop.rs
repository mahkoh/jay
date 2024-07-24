mod remote_desktop_gui;

use {
    crate::{
        dbus::{prelude::Variant, DbusObject, DictEntry, DynamicType, PendingReply, FALSE},
        ifs::jay_compositor::CREATE_EI_SESSION_SINCE,
        portal::{
            ptl_display::{PortalDisplay, PortalDisplayId},
            ptl_remote_desktop::remote_desktop_gui::SelectionGui,
            PortalState, PORTAL_SUCCESS,
        },
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
        },
        wire_dbus::{
            org,
            org::freedesktop::impl_::portal::{
                remote_desktop::{
                    ConnectToEIS, ConnectToEISReply, CreateSession, CreateSessionReply,
                    SelectDevices, SelectDevicesReply, Start, StartReply,
                },
                session::{CloseReply as SessionCloseReply, Closed},
            },
        },
        wl_usr::usr_ifs::usr_jay_ei_session::{UsrJayEiSession, UsrJayEiSessionOwner},
    },
    std::{borrow::Cow, cell::Cell, ops::Deref, rc::Rc},
    uapi::OwnedFd,
};

shared_ids!(ScreencastSessionId);
pub struct RemoteDesktopSession {
    _id: ScreencastSessionId,
    state: Rc<PortalState>,
    pub app: String,
    session_obj: DbusObject,
    pub phase: CloneCell<RemoteDesktopPhase>,
}

#[derive(Clone)]
pub enum RemoteDesktopPhase {
    Init,
    DevicesSelected,
    Selecting(Rc<SelectingDisplay>),
    Starting(Rc<StartingRemoteDesktop>),
    Started(Rc<StartedRemoteDesktop>),
    Terminated,
}

unsafe impl UnsafeCellCloneSafe for RemoteDesktopPhase {}

pub struct SelectingDisplay {
    pub session: Rc<RemoteDesktopSession>,
    pub request_obj: Rc<DbusObject>,
    pub reply: Rc<PendingReply<StartReply<'static>>>,
    pub guis: CopyHashMap<PortalDisplayId, Rc<SelectionGui>>,
}

pub struct StartingRemoteDesktop {
    pub session: Rc<RemoteDesktopSession>,
    pub _request_obj: Rc<DbusObject>,
    pub reply: Rc<PendingReply<StartReply<'static>>>,
    pub dpy: Rc<PortalDisplay>,
    pub ei_session: Rc<UsrJayEiSession>,
}

pub struct StartedRemoteDesktop {
    session: Rc<RemoteDesktopSession>,
    dpy: Rc<PortalDisplay>,
    ei_session: Rc<UsrJayEiSession>,
    ei_fd: Cell<Option<Rc<OwnedFd>>>,
}

bitflags! {
    DeviceTypes: u32;

    KEYBOARD = 1,
    POINTER = 2,
    TOUCHSCREEN = 4,
}

impl UsrJayEiSessionOwner for StartingRemoteDesktop {
    fn created(&self, fd: &Rc<OwnedFd>) {
        {
            let inner_type = DynamicType::DictEntry(
                Box::new(DynamicType::String),
                Box::new(DynamicType::Variant),
            );
            let kt = DynamicType::Struct(vec![
                DynamicType::U32,
                DynamicType::Array(Box::new(inner_type.clone())),
            ]);
            let variants = [
                DictEntry {
                    key: "devices".into(),
                    value: Variant::U32(DeviceTypes::all().0),
                },
                DictEntry {
                    key: "clipboard_enabled".into(),
                    value: Variant::Bool(FALSE),
                },
                DictEntry {
                    key: "streams".into(),
                    value: Variant::Array(kt, vec![]),
                },
            ];
            self.reply.ok(&StartReply {
                response: PORTAL_SUCCESS,
                results: Cow::Borrowed(&variants[..]),
            });
        }
        let started = Rc::new(StartedRemoteDesktop {
            session: self.session.clone(),
            dpy: self.dpy.clone(),
            ei_session: self.ei_session.clone(),
            ei_fd: Cell::new(Some(fd.clone())),
        });
        self.session
            .phase
            .set(RemoteDesktopPhase::Started(started.clone()));
        started.ei_session.owner.set(Some(started.clone()));
    }

    fn failed(&self, reason: &str) {
        log::error!("Could not create session: {}", reason);
        self.reply.err(reason);
        self.session.kill();
    }
}

impl SelectingDisplay {
    pub fn starting(&self, dpy: &Rc<PortalDisplay>) {
        let builder = dpy.jc.create_ei_session();
        builder.set_app_id(&self.session.app);
        let ei_session = builder.commit();
        let starting = Rc::new(StartingRemoteDesktop {
            session: self.session.clone(),
            _request_obj: self.request_obj.clone(),
            reply: self.reply.clone(),
            dpy: dpy.clone(),
            ei_session,
        });
        self.session
            .phase
            .set(RemoteDesktopPhase::Starting(starting.clone()));
        starting.ei_session.owner.set(Some(starting.clone()));
        dpy.remote_desktop_sessions.set(
            self.session.session_obj.path().to_owned(),
            self.session.clone(),
        );
    }
}

impl RemoteDesktopSession {
    pub(super) fn kill(&self) {
        self.session_obj.emit_signal(&Closed);
        self.state
            .remote_desktop_sessions
            .remove(self.session_obj.path());
        match self.phase.set(RemoteDesktopPhase::Terminated) {
            RemoteDesktopPhase::Init => {}
            RemoteDesktopPhase::DevicesSelected => {}
            RemoteDesktopPhase::Terminated => {}
            RemoteDesktopPhase::Selecting(s) => {
                s.reply.err("Session has been terminated");
                for gui in s.guis.lock().drain_values() {
                    gui.kill(false);
                }
            }
            RemoteDesktopPhase::Starting(s) => {
                s.reply.err("Session has been terminated");
                s.ei_session.con.remove_obj(s.ei_session.deref());
                s.dpy
                    .remote_desktop_sessions
                    .remove(self.session_obj.path());
            }
            RemoteDesktopPhase::Started(s) => {
                s.ei_session.con.remove_obj(s.ei_session.deref());
                s.dpy
                    .remote_desktop_sessions
                    .remove(self.session_obj.path());
            }
        }
    }

    fn dbus_select_devices(
        self: &Rc<Self>,
        _req: SelectDevices,
        reply: PendingReply<SelectDevicesReply<'static>>,
    ) {
        match self.phase.get() {
            RemoteDesktopPhase::Init => {}
            _ => {
                self.kill();
                reply.err("Devices have already been selected");
                return;
            }
        }
        self.phase.set(RemoteDesktopPhase::DevicesSelected);
        reply.ok(&SelectDevicesReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }

    fn dbus_start(self: &Rc<Self>, req: Start<'_>, reply: PendingReply<StartReply<'static>>) {
        match self.phase.get() {
            RemoteDesktopPhase::DevicesSelected => {}
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
            if dpy.outputs.len() > 0 && dpy.jc.version >= CREATE_EI_SESSION_SINCE {
                guis.set(dpy.id, SelectionGui::new(self, dpy));
            }
        }
        if guis.is_empty() {
            self.kill();
            reply.err("There are no running displays");
            return;
        }
        self.phase
            .set(RemoteDesktopPhase::Selecting(Rc::new(SelectingDisplay {
                session: self.clone(),
                request_obj: Rc::new(request_obj),
                reply: Rc::new(reply),
                guis,
            })));
    }

    fn dbus_connect_to_eis(
        self: &Rc<Self>,
        _req: ConnectToEIS,
        reply: PendingReply<ConnectToEISReply>,
    ) {
        let RemoteDesktopPhase::Started(started) = self.phase.get() else {
            self.kill();
            reply.err("Sources have already been selected");
            return;
        };
        let Some(fd) = started.ei_fd.take() else {
            self.kill();
            reply.err("EI file descriptor has already been consumed");
            return;
        };
        reply.ok(&ConnectToEISReply { fd });
    }
}

impl UsrJayEiSessionOwner for StartedRemoteDesktop {
    fn destroyed(&self) {
        self.session.kill();
    }
}

pub(super) fn add_remote_desktop_dbus_members(state_: &Rc<PortalState>, object: &DbusObject) {
    use org::freedesktop::impl_::portal::remote_desktop::*;
    let state = state_.clone();
    object.add_method::<CreateSession, _>(move |req, pr| {
        dbus_create_session(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<SelectDevices, _>(move |req, pr| {
        dbus_select_devices(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<Start, _>(move |req, pr| {
        dbus_start(&state, req, pr);
    });
    let state = state_.clone();
    object.add_method::<ConnectToEIS, _>(move |req, pr| {
        dbus_connect_to_eis(&state, req, pr);
    });
    object.set_property::<AvailableDeviceTypes>(Variant::U32(DeviceTypes::all().0));
    object.set_property::<version>(Variant::U32(2));
}

fn dbus_create_session(
    state: &Rc<PortalState>,
    req: CreateSession,
    reply: PendingReply<CreateSessionReply<'static>>,
) {
    log::info!("Create remote desktop session {:#?}", req);
    if state
        .remote_desktop_sessions
        .contains(req.session_handle.0.deref())
    {
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
    let session = Rc::new(RemoteDesktopSession {
        _id: state.id(),
        state: state.clone(),
        app: req.app_id.to_string(),
        session_obj: obj,
        phase: CloneCell::new(RemoteDesktopPhase::Init),
    });
    {
        use org::freedesktop::impl_::portal::session::*;
        let ses = session.clone();
        session.session_obj.add_method::<Close, _>(move |_, pr| {
            ses.kill();
            pr.ok(&SessionCloseReply);
        });
        session.session_obj.set_property::<version>(Variant::U32(2));
    }
    state
        .remote_desktop_sessions
        .set(req.session_handle.0.to_string(), session);
    reply.ok(&CreateSessionReply {
        response: PORTAL_SUCCESS,
        results: Default::default(),
    });
}

fn dbus_select_devices(
    state: &Rc<PortalState>,
    req: SelectDevices,
    reply: PendingReply<SelectDevicesReply<'static>>,
) {
    if let Some(s) = get_session(state, &reply, &req.session_handle.0) {
        s.dbus_select_devices(req, reply);
    }
}

fn dbus_start(state: &Rc<PortalState>, req: Start, reply: PendingReply<StartReply<'static>>) {
    if let Some(s) = get_session(state, &reply, &req.session_handle.0) {
        s.dbus_start(req, reply);
    }
}

fn dbus_connect_to_eis(
    state: &Rc<PortalState>,
    req: ConnectToEIS,
    reply: PendingReply<ConnectToEISReply>,
) {
    if let Some(s) = get_session(state, &reply, &req.session_handle.0) {
        s.dbus_connect_to_eis(req, reply);
    }
}

fn get_session<T>(
    state: &Rc<PortalState>,
    reply: &PendingReply<T>,
    handle: &str,
) -> Option<Rc<RemoteDesktopSession>> {
    let res = state.remote_desktop_sessions.get(handle);
    if res.is_none() {
        let msg = format!("Remote desktop session `{}` does not exist", handle);
        reply.err(&msg);
    }
    res
}

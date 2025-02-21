mod remote_desktop_gui;

use {
    crate::{
        dbus::{DbusObject, PendingReply, prelude::Variant},
        ifs::jay_compositor::CREATE_EI_SESSION_SINCE,
        portal::{
            PORTAL_SUCCESS, PortalState,
            ptl_display::{PortalDisplay, PortalDisplayId},
            ptl_remote_desktop::remote_desktop_gui::SelectionGui,
            ptl_screencast::ScreencastPhase,
            ptl_session::{PortalSession, PortalSessionReply},
        },
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
        },
        wire_dbus::{
            org,
            org::freedesktop::impl_::portal::{
                remote_desktop::{
                    ConnectToEIS, ConnectToEISReply, CreateSession, CreateSessionReply,
                    SelectDevices, SelectDevicesReply, Start, StartReply,
                },
                session::CloseReply as SessionCloseReply,
            },
        },
        wl_usr::usr_ifs::usr_jay_ei_session::{UsrJayEiSession, UsrJayEiSessionOwner},
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
    uapi::OwnedFd,
};

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
    pub session: Rc<PortalSession>,
    pub request_obj: Rc<DbusObject>,
    pub guis: CopyHashMap<PortalDisplayId, Rc<SelectionGui>>,
}

pub struct StartingRemoteDesktop {
    pub session: Rc<PortalSession>,
    pub request_obj: Rc<DbusObject>,
    pub dpy: Rc<PortalDisplay>,
    pub ei_session: Rc<UsrJayEiSession>,
}

pub struct StartedRemoteDesktop {
    pub session: Rc<PortalSession>,
    pub dpy: Rc<PortalDisplay>,
    pub ei_session: Rc<UsrJayEiSession>,
    pub ei_fd: Cell<Option<Rc<OwnedFd>>>,
}

bitflags! {
    DeviceTypes: u32;

    KEYBOARD = 1,
    POINTER = 2,
    TOUCHSCREEN = 4,
}

impl UsrJayEiSessionOwner for StartingRemoteDesktop {
    fn created(&self, fd: &Rc<OwnedFd>) {
        let started = Rc::new(StartedRemoteDesktop {
            session: self.session.clone(),
            dpy: self.dpy.clone(),
            ei_session: self.ei_session.clone(),
            ei_fd: Cell::new(Some(fd.clone())),
        });
        self.session
            .rd_phase
            .set(RemoteDesktopPhase::Started(started.clone()));
        started.ei_session.owner.set(Some(started.clone()));
        if let ScreencastPhase::SourcesSelected(s) = self.session.sc_phase.get() {
            self.session.screencast_restore(
                &self.request_obj,
                s.restore_data.take(),
                Some(self.dpy.clone()),
            );
        } else {
            self.session.send_start_reply(None, None, None);
        }
    }

    fn failed(&self, reason: &str) {
        log::error!("Could not create session: {}", reason);
        self.session.reply_err(reason);
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
            request_obj: self.request_obj.clone(),
            dpy: dpy.clone(),
            ei_session,
        });
        self.session
            .rd_phase
            .set(RemoteDesktopPhase::Starting(starting.clone()));
        starting.ei_session.owner.set(Some(starting.clone()));
        dpy.sessions.set(
            self.session.session_obj.path().to_owned(),
            self.session.clone(),
        );
    }
}

impl PortalSession {
    fn dbus_select_devices(
        self: &Rc<Self>,
        _req: SelectDevices,
        reply: PendingReply<SelectDevicesReply<'static>>,
    ) {
        match self.rd_phase.get() {
            RemoteDesktopPhase::Init => {}
            _ => {
                self.kill();
                reply.err("Devices have already been selected");
                return;
            }
        }
        self.rd_phase.set(RemoteDesktopPhase::DevicesSelected);
        reply.ok(&SelectDevicesReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }

    fn dbus_start_remote_desktop(
        self: &Rc<Self>,
        req: Start<'_>,
        reply: PendingReply<StartReply<'static>>,
    ) {
        match self.rd_phase.get() {
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
        self.start_reply
            .set(Some(PortalSessionReply::RemoteDesktop(reply)));
        self.rd_phase
            .set(RemoteDesktopPhase::Selecting(Rc::new(SelectingDisplay {
                session: self.clone(),
                request_obj: Rc::new(request_obj),
                guis,
            })));
    }

    fn dbus_connect_to_eis(
        self: &Rc<Self>,
        _req: ConnectToEIS,
        reply: PendingReply<ConnectToEISReply>,
    ) {
        let RemoteDesktopPhase::Started(started) = self.rd_phase.get() else {
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
        pw_con: state.pw_con.clone(),
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
        session.session_obj.set_property::<version>(Variant::U32(2));
    }
    state
        .sessions
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
        s.dbus_start_remote_desktop(req, reply);
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
) -> Option<Rc<PortalSession>> {
    let res = state.sessions.get(handle);
    if res.is_none() {
        let msg = format!("Remote desktop session `{}` does not exist", handle);
        reply.err(&msg);
    }
    res
}

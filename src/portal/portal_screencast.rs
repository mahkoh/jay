use {
    crate::{
        dbus::{prelude::Variant, DbusObject, PendingReply},
        portal::{PortalState, PORTAL_SUCCESS},
        utils::copyhashmap::CopyHashMap,
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
    },
    std::{ops::Deref, rc::Rc},
};

#[derive(Default)]
pub struct PortalScreencastsState {
    sessions: CopyHashMap<String, Rc<ScreencastSession>>,
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
    let session = Rc::new(ScreencastSession {
        state: state.clone(),
        path: req.session_handle.0.to_string(),
        app: req.app_id.to_string(),
        obj,
    });
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
    session.state.screencasts.sessions.remove(&session.path);
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
    if let Some(reply) = reply {
        reply.ok(&SelectSourcesReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }
}

fn start(state: &Rc<PortalState>, req: Start, reply: Option<PendingReply<StartReply<'static>>>) {
    log::info!("{:#?}", req);
    if let Some(reply) = reply {
        reply.ok(&StartReply {
            response: PORTAL_SUCCESS,
            results: Default::default(),
        });
    }
}

struct ScreencastSession {
    state: Rc<PortalState>,
    path: String,
    app: String,
    obj: DbusObject,
}

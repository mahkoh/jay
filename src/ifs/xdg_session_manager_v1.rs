use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::xdg_session_v1::XdgSessionV1,
        leaks::Tracker,
        object::{Object, Version},
        sm::{SessionGetStatus, SessionManager, SessionName, SessionReason, session_name},
        state::State,
        wire::{XdgSessionManagerV1Id, xdg_session_manager_v1::*},
    },
    std::{rc::Rc, str::FromStr},
    thiserror::Error,
};

pub const REASON_LAUNCH: u32 = 1;
pub const REASON_RECOVER: u32 = 2;
const REASON_SESSION_RESTORE: u32 = 3;

pub struct XdgSessionManagerV1Global {
    name: GlobalName,
}

impl XdgSessionManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgSessionManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XdgSessionManagerV1Error> {
        let Some(sm) = &client.state.sm else {
            return Err(XdgSessionManagerV1Error::SessionManagerNotAvailable);
        };
        let obj = Rc::new(XdgSessionManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            sm: sm.clone(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    XdgSessionManagerV1Global,
    XdgSessionManagerV1,
    XdgSessionManagerV1Error
);

impl Global for XdgSessionManagerV1Global {
    fn version(&self) -> u32 {
        1
    }

    fn exposed(&self, state: &State) -> bool {
        state.sm.is_some()
    }
}

simple_add_global!(XdgSessionManagerV1Global);

pub struct XdgSessionManagerV1 {
    pub id: XdgSessionManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub sm: Rc<SessionManager>,
}

const MAX_LIVE_SESSIONS: usize = 32;

impl XdgSessionManagerV1RequestHandler for XdgSessionManagerV1 {
    type Error = XdgSessionManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_session(&self, req: GetSession<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let reason = match req.reason {
            REASON_LAUNCH => SessionReason::Launch,
            REASON_RECOVER => SessionReason::Recover,
            REASON_SESSION_RESTORE => SessionReason::SessionRestore,
            n => return Err(XdgSessionManagerV1Error::UnknownReason(n)),
        };
        let name = req
            .session_id
            .and_then(|s| SessionName::from_str(s).ok())
            .unwrap_or_else(session_name);
        let obj = Rc::new(XdgSessionV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            session: Default::default(),
            name,
            link: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        let (session, status) = self
            .sm
            .get(name, req.session_id.is_some(), reason, obj.clone());
        obj.session.set(Some(session));
        if self.client.num_live_sessions.fetch_add(1) >= MAX_LIVE_SESSIONS
            && let Some(old) = self.client.live_sessions.first()
        {
            old.disown_to_peer();
            old.send_replaced();
        }
        *obj.link.borrow_mut() = Some(self.client.live_sessions.add_last(obj.clone()));
        if let Some(status) = status {
            match status {
                SessionGetStatus::Created => obj.send_created(name),
                SessionGetStatus::Restored => obj.send_restored(),
            }
        }
        Ok(())
    }
}

object_base! {
    self = XdgSessionManagerV1;
    version = self.version;
}

impl Object for XdgSessionManagerV1 {}

simple_add_obj!(XdgSessionManagerV1);

#[derive(Debug, Error)]
pub enum XdgSessionManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The session manager is not available")]
    SessionManagerNotAvailable,
    #[error("Unknown reason {0}")]
    UnknownReason(u32),
}
efrom!(XdgSessionManagerV1Error, ClientError);

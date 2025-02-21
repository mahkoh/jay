use {
    crate::{
        client::{Client, ClientError},
        ei::ei_client::EiClientError,
        ifs::jay_ei_session::JayEiSession,
        leaks::Tracker,
        object::{Object, Version},
        utils::{errorfmt::ErrorFmt, oserror::OsError},
        wire::{
            JayEiSessionBuilderId,
            jay_ei_session_builder::{Commit, JayEiSessionBuilderRequestHandler, SetAppId},
        },
    },
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
    uapi::c,
};

pub struct JayEiSessionBuilder {
    pub id: JayEiSessionBuilderId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub app_id: RefCell<Option<String>>,
}

impl JayEiSessionBuilderRequestHandler for JayEiSessionBuilder {
    type Error = JayEiSessionBuilderError;

    fn commit(&self, req: Commit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        let app_id = self.app_id.borrow().clone();
        if app_id.is_none() {
            return Err(JayEiSessionBuilderError::NoAppId);
        }
        let res = (move || {
            let con = uapi::socketpair(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0);
            let (server, client) = match con {
                Ok(w) => w,
                Err(e) => return Err(JayEiSessionBuilderError::SocketPair(e.into())),
            };
            let ei_client_id = self
                .client
                .state
                .ei_clients
                .spawn2(&self.client.state, Rc::new(server), None, app_id)
                .map_err(JayEiSessionBuilderError::SpawnClient)?
                .id;
            Ok((ei_client_id, Rc::new(client)))
        })();
        let obj = Rc::new(JayEiSession {
            id: req.id,
            client: self.client.clone(),
            ei_client_id: res.as_ref().ok().map(|v| v.0),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        match res {
            Ok((_, fd)) => obj.send_created(&fd),
            Err(e) => {
                let e = format!("Could not spawn client: {}", ErrorFmt(e));
                log::error!("{}", e);
                obj.send_failed(&e);
            }
        }
        Ok(())
    }

    fn set_app_id(&self, req: SetAppId<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.app_id.borrow_mut() = Some(req.app_id.to_string());
        Ok(())
    }
}

object_base! {
    self = JayEiSessionBuilder;
    version = self.version;
}

impl Object for JayEiSessionBuilder {}

simple_add_obj!(JayEiSessionBuilder);

#[derive(Debug, Error)]
pub enum JayEiSessionBuilderError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not create a socketpair")]
    SocketPair(#[source] OsError),
    #[error("Could not spawn a new client")]
    SpawnClient(#[source] EiClientError),
    #[error("Commit called without app-id")]
    NoAppId,
}
efrom!(JayEiSessionBuilderError, ClientError);

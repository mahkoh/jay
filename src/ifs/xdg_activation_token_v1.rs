use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::activation_token::{ActivationToken, activation_token},
        wire::{XdgActivationTokenV1Id, xdg_activation_token_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

const MAX_TOKENS_PER_CLIENT: usize = 8;

pub struct XdgActivationTokenV1 {
    pub id: XdgActivationTokenV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    already_used: Cell<bool>,
    version: Version,
}

impl XdgActivationTokenV1 {
    pub fn new(id: XdgActivationTokenV1Id, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            already_used: Cell::new(false),
            version,
        }
    }
}

impl XdgActivationTokenV1RequestHandler for XdgActivationTokenV1 {
    type Error = XdgActivationTokenV1Error;

    fn set_serial(&self, _req: SetSerial, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_app_id(&self, _req: SetAppId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_surface(&self, req: SetSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.lookup(req.surface)?;
        Ok(())
    }

    fn commit(&self, _req: Commit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.already_used.replace(true) {
            return Err(XdgActivationTokenV1Error::AlreadyUsed);
        }
        let token = activation_token();
        self.client.state.activation_tokens.set(token, ());
        let mut tokens = self.client.activation_tokens.borrow_mut();
        if tokens.len() >= MAX_TOKENS_PER_CLIENT
            && let Some(oldest) = tokens.pop_front()
        {
            self.client.state.activation_tokens.remove(&oldest);
        }
        tokens.push_back(token);
        self.send_done(token);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl XdgActivationTokenV1 {
    fn send_done(&self, token: ActivationToken) {
        let token = token.to_string();
        self.client.event(Done {
            self_id: self.id,
            token: &token,
        });
    }
}

object_base! {
    self = XdgActivationTokenV1;
    version = self.version;
}

impl Object for XdgActivationTokenV1 {}

simple_add_obj!(XdgActivationTokenV1);

#[derive(Debug, Error)]
pub enum XdgActivationTokenV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The activation token has already been used")]
    AlreadyUsed,
}
efrom!(XdgActivationTokenV1Error, ClientError);

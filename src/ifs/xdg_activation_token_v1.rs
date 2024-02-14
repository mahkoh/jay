use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        utils::{
            activation_token::{activation_token, ActivationToken},
            buffd::{MsgParser, MsgParserError},
        },
        wire::{xdg_activation_token_v1::*, XdgActivationTokenV1Id},
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
}

impl XdgActivationTokenV1 {
    pub fn new(id: XdgActivationTokenV1Id, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            already_used: Cell::new(false),
        }
    }

    fn set_serial(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgActivationTokenV1Error> {
        let _req: SetSerial = self.client.parse(self, parser)?;
        Ok(())
    }

    fn set_app_id(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgActivationTokenV1Error> {
        let _req: SetAppId = self.client.parse(self, parser)?;
        Ok(())
    }

    fn set_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgActivationTokenV1Error> {
        let req: SetSurface = self.client.parse(self, parser)?;
        self.client.lookup(req.surface)?;
        Ok(())
    }

    fn commit(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgActivationTokenV1Error> {
        let _req: Commit = self.client.parse(self, parser)?;
        if self.already_used.replace(true) {
            return Err(XdgActivationTokenV1Error::AlreadyUsed);
        }
        let token = activation_token();
        self.client.state.activation_tokens.set(token, ());
        let mut tokens = self.client.activation_tokens.borrow_mut();
        if tokens.len() >= MAX_TOKENS_PER_CLIENT {
            if let Some(oldest) = tokens.pop_front() {
                self.client.state.activation_tokens.remove(&oldest);
            }
        }
        tokens.push_back(token);
        self.send_done(token);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgActivationTokenV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

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

    SET_SERIAL => set_serial,
    SET_APP_ID => set_app_id,
    SET_SURFACE => set_surface,
    COMMIT => commit,
    DESTROY => destroy,
}

impl Object for XdgActivationTokenV1 {}

simple_add_obj!(XdgActivationTokenV1);

#[derive(Debug, Error)]
pub enum XdgActivationTokenV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("The activation token has already been used")]
    AlreadyUsed,
}
efrom!(XdgActivationTokenV1Error, ClientError);
efrom!(XdgActivationTokenV1Error, MsgParserError);

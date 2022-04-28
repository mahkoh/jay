use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{org_kde_kwin_server_decoration::*, OrgKdeKwinServerDecorationId},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

#[allow(dead_code)]
const NONE: u32 = 0;
#[allow(dead_code)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

pub struct OrgKdeKwinServerDecoration {
    id: OrgKdeKwinServerDecorationId,
    client: Rc<Client>,
    requested: Cell<bool>,
    pub tracker: Tracker<Self>,
}

impl OrgKdeKwinServerDecoration {
    pub fn new(id: OrgKdeKwinServerDecorationId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            requested: Cell::new(false),
            tracker: Default::default(),
        }
    }

    pub fn send_mode(self: &Rc<Self>, mode: u32) {
        self.client.event(Mode {
            self_id: self.id,
            mode,
        })
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), OrgKdeKwinServerDecorationError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn request_mode(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), OrgKdeKwinServerDecorationError> {
        let req: RequestMode = self.client.parse(&**self, parser)?;
        if req.mode > SERVER {
            return Err(OrgKdeKwinServerDecorationError::InvalidMode(req.mode));
        }
        let mode = if self.requested.replace(true) {
            req.mode
        } else {
            SERVER
        };
        self.send_mode(mode);
        Ok(())
    }
}

object_base! {
    OrgKdeKwinServerDecoration;

    RELEASE => release,
    REQUEST_MODE => request_mode,
}

impl Object for OrgKdeKwinServerDecoration {
    fn num_requests(&self) -> u32 {
        REQUEST_MODE + 1
    }
}

simple_add_obj!(OrgKdeKwinServerDecoration);

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Mode {0} does not exist")]
    InvalidMode(u32),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(OrgKdeKwinServerDecorationError, ClientError);
efrom!(OrgKdeKwinServerDecorationError, MsgParserError);

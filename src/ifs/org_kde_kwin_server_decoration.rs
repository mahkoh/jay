use crate::client::{Client, ClientError, DynEventFormatter};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;
use crate::wire::org_kde_kwin_server_decoration::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::OrgKdeKwinServerDecorationId;

#[allow(dead_code)]
const NONE: u32 = 0;
#[allow(dead_code)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

pub struct OrgKdeKwinServerDecoration {
    id: OrgKdeKwinServerDecorationId,
    client: Rc<Client>,
    requested: Cell<bool>,
}

impl OrgKdeKwinServerDecoration {
    pub fn new(id: OrgKdeKwinServerDecorationId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            requested: Cell::new(false),
        }
    }

    pub fn mode(self: &Rc<Self>, mode: u32) -> DynEventFormatter {
        Box::new(Mode {
            self_id: self.id,
            mode,
        })
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn request_mode(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), RequestModeError> {
        let req: RequestMode = self.client.parse(&**self, parser)?;
        if req.mode > SERVER {
            return Err(RequestModeError::InvalidMode(req.mode));
        }
        let mode = if self.requested.replace(true) {
            req.mode
        } else {
            SERVER
        };
        self.client.event(self.mode(mode));
        Ok(())
    }
}

object_base! {
    OrgKdeKwinServerDecoration, OrgKdeKwinServerDecorationError;

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
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error("Could not process a `request_mode` request")]
    RequestModeError(#[from] RequestModeError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OrgKdeKwinServerDecorationError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(ReleaseError, ClientError);
efrom!(ReleaseError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum RequestModeError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Mode {0} does not exist")]
    InvalidMode(u32),
}
efrom!(RequestModeError, ClientError);
efrom!(RequestModeError, ParseError, MsgParserError);

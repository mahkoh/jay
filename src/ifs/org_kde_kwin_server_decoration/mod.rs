use crate::client::{Client, DynEventFormatter};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::cell::Cell;
use std::rc::Rc;
pub use types::*;

mod types;

const RELEASE: u32 = 0;
const REQUEST_MODE: u32 = 1;

const MODE: u32 = 0;

#[allow(dead_code)]
const NONE: u32 = 0;
#[allow(dead_code)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

id!(OrgKdeKwinServerDecorationId);

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
            obj: self.clone(),
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

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), OrgKdeKwinServerDecorationError> {
        match request {
            RELEASE => self.release(parser)?,
            REQUEST_MODE => self.request_mode(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(OrgKdeKwinServerDecoration);

impl Object for OrgKdeKwinServerDecoration {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::OrgKdeKwinServerDecoration
    }

    fn num_requests(&self) -> u32 {
        REQUEST_MODE + 1
    }
}

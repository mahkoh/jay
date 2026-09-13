use crate::client::Client;
use crate::client::LookupError;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::OrgKdeKwinServerDecorationId;
use crate::wire::org_kde_kwin_server_decoration::*;
use jay_proc::Object;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

#[expect(unused)]
const NONE: u32 = 0;
#[expect(unused)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

#[derive(Object)]
pub struct OrgKdeKwinServerDecoration {
    id: OrgKdeKwinServerDecorationId,
    client: Rc<Client>,
    requested: Cell<bool>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl OrgKdeKwinServerDecoration {
    pub fn new(id: OrgKdeKwinServerDecorationId, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            requested: Cell::new(false),
            tracker: Default::default(),
            version,
        }
    }

    pub fn send_mode(&self, mode: u32) {
        self.client.event(Mode {
            self_id: self.id,
            mode,
        })
    }
}

impl OrgKdeKwinServerDecorationRequestHandler for OrgKdeKwinServerDecoration {
    type Error = OrgKdeKwinServerDecorationError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn request_mode(&self, req: RequestMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationError {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("Mode {0} does not exist")]
    InvalidMode(u32),
}

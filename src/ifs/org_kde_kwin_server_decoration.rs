use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{OrgKdeKwinServerDecorationId, org_kde_kwin_server_decoration::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

#[expect(dead_code)]
const NONE: u32 = 0;
#[expect(dead_code)]
const CLIENT: u32 = 1;
const SERVER: u32 = 2;

pub struct OrgKdeKwinServerDecoration {
    id: OrgKdeKwinServerDecorationId,
    client: Rc<Client>,
    requested: Cell<bool>,
    pub tracker: Tracker<Self>,
    pub version: Version,
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
        self.client.remove_obj(self)?;
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

object_base! {
    self = OrgKdeKwinServerDecoration;
    version = self.version;
}

impl Object for OrgKdeKwinServerDecoration {}

simple_add_obj!(OrgKdeKwinServerDecoration);

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Mode {0} does not exist")]
    InvalidMode(u32),
}
efrom!(OrgKdeKwinServerDecorationError, ClientError);

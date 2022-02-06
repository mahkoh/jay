use crate::client::{Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_drm::*;
use crate::wire::WlDrmId;
use bstr::ByteSlice;
use std::ffi::CString;
use std::rc::Rc;
use thiserror::Error;

const PRIME: u32 = 1;

pub struct WlDrmGlobal {
    name: GlobalName,
}

impl WlDrmGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlDrmId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlDrmError> {
        let obj = Rc::new(WlDrm {
            id,
            client: client.clone(),
            _version: version,
        });
        client.add_client_obj(&obj)?;
        if let Some(rc) = client.state.render_ctx.get() {
            obj.send_device(&rc.render_node());
            obj.send_capabilities(PRIME);
        }
        Ok(())
    }
}

global_base!(WlDrmGlobal, WlDrm, WlDrmError);

impl Global for WlDrmGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WlDrmGlobal);

pub struct WlDrm {
    id: WlDrmId,
    pub client: Rc<Client>,
    _version: u32,
}

impl WlDrm {
    fn send_device(self: &Rc<Self>, device: &Rc<CString>) {
        self.client.event(Device {
            self_id: self.id,
            name: device.as_bytes().as_bstr(),
        })
    }

    fn send_authenticated(self: &Rc<Self>) {
        self.client.event(Authenticated { self_id: self.id })
    }

    fn send_capabilities(self: &Rc<Self>, value: u32) {
        self.client.event(Capabilities {
            self_id: self.id,
            value,
        })
    }

    fn authenticate(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), AuthenticateError> {
        let _req: Authenticate = self.client.parse(&**self, parser)?;
        self.send_authenticated();
        Ok(())
    }

    fn create_buffer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CreateBufferError> {
        let _req: CreateBuffer = self.client.parse(&**self, parser)?;
        Err(CreateBufferError::Unsupported)
    }

    fn create_planar_buffer(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), CreatePlanarBufferError> {
        let _req: CreatePlanarBuffer = self.client.parse(&**self, parser)?;
        Err(CreatePlanarBufferError::Unsupported)
    }
}

object_base! {
    WlDrm, WlDrmError;

    AUTHENTICATE => authenticate,
    CREATE_BUFFER => create_buffer,
    CREATE_PLANAR_BUFFER => create_planar_buffer,
}

impl Object for WlDrm {
    fn num_requests(&self) -> u32 {
        CREATE_PLANAR_BUFFER + 1
    }
}

simple_add_obj!(WlDrm);

#[derive(Debug, Error)]
pub enum WlDrmError {
    #[error("Could not process a `authenticate` request")]
    AuthenticateError(#[from] AuthenticateError),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `create_planar_buffer` request")]
    CreatePlanarBufferError(#[from] CreatePlanarBufferError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlDrmError, ClientError);

#[derive(Debug, Error)]
pub enum AuthenticateError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(AuthenticateError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreateBufferError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreatePlanarBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreatePlanarBufferError, ParseError, MsgParserError);

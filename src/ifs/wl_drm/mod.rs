use crate::client::{Client, DynEventFormatter};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::ffi::CString;
use std::rc::Rc;
pub use types::*;

mod types;

id!(WlDrmId);

const AUTHENTICATE: u32 = 0;
const CREATE_BUFFER: u32 = 1;
const CREATE_PLANAR_BUFFER: u32 = 2;

const DEVICE: u32 = 0;
const FORMAT: u32 = 1;
const AUTHENTICATED: u32 = 2;
const CAPABILITIES: u32 = 3;

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
        let obj = Rc::new(WlDrmObj {
            id,
            client: client.clone(),
            _version: version,
        });
        client.add_client_obj(&obj)?;
        if let Some(rc) = client.state.render_ctx.get() {
            client.event(obj.device(&rc.render_node()));
            client.event(obj.capabilities(PRIME));
        }
        Ok(())
    }
}

bind!(WlDrmGlobal);

impl Global for WlDrmGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn singleton(&self) -> bool {
        true
    }

    fn interface(&self) -> Interface {
        Interface::WlDrm
    }

    fn version(&self) -> u32 {
        1
    }
}

pub struct WlDrmObj {
    id: WlDrmId,
    pub client: Rc<Client>,
    _version: u32,
}

impl WlDrmObj {
    fn device(self: &Rc<Self>, device: &Rc<CString>) -> DynEventFormatter {
        Box::new(Device {
            obj: self.clone(),
            name: device.clone(),
        })
    }

    fn authenticated(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Authenticated { obj: self.clone() })
    }

    fn capabilities(self: &Rc<Self>, value: u32) -> DynEventFormatter {
        Box::new(Capabilities {
            obj: self.clone(),
            value,
        })
    }

    fn authenticate(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), AuthenticateError> {
        let _req: Authenticate = self.client.parse(&**self, parser)?;
        self.client.event(self.authenticated());
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

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDrmError> {
        match request {
            AUTHENTICATE => self.authenticate(parser)?,
            CREATE_BUFFER => self.create_buffer(parser)?,
            CREATE_PLANAR_BUFFER => self.create_planar_buffer(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDrmObj);

impl Object for WlDrmObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDrm
    }

    fn num_requests(&self) -> u32 {
        CREATE_PLANAR_BUFFER + 1
    }
}

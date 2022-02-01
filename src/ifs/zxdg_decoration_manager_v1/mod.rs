use crate::client::Client;
use crate::globals::{Global, GlobalName};
use crate::ifs::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

mod types;

const DESTROY: u32 = 0;
const GET_TOPLEVEL_DECORATION: u32 = 1;

id!(ZxdgDecorationManagerV1Id);

pub struct ZxdgDecorationManagerV1Global {
    name: GlobalName,
}
impl ZxdgDecorationManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZxdgDecorationManagerV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), ZxdgDecorationManagerV1Error> {
        let obj = Rc::new(ZxdgDecorationManagerV1Obj {
            id,
            client: client.clone(),
            _version: version,
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

bind!(ZxdgDecorationManagerV1Global);

impl Global for ZxdgDecorationManagerV1Global {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn singleton(&self) -> bool {
        true
    }

    fn interface(&self) -> Interface {
        Interface::ZxdgDecorationManagerV1
    }

    fn version(&self) -> u32 {
        1
    }
}

pub struct ZxdgDecorationManagerV1Obj {
    id: ZxdgDecorationManagerV1Id,
    client: Rc<Client>,
    _version: u32,
}

impl ZxdgDecorationManagerV1Obj {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_toplevel_decoration(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), GetToplevelDecorationError> {
        let req: GetToplevelDecoration = self.client.parse(self, parser)?;
        let tl = self.client.get_xdg_toplevel(req.toplevel)?;
        let obj = Rc::new(ZxdgToplevelDecorationV1::new(req.id, &self.client, &tl));
        self.client.add_client_obj(&obj)?;
        obj.send_configure();
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZxdgDecorationManagerV1Error> {
        match request {
            DESTROY => self.destroy(parser)?,
            GET_TOPLEVEL_DECORATION => self.get_toplevel_decoration(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(ZxdgDecorationManagerV1Obj);

impl Object for ZxdgDecorationManagerV1Obj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::ZxdgDecorationManagerV1
    }

    fn num_requests(&self) -> u32 {
        GET_TOPLEVEL_DECORATION + 1
    }
}

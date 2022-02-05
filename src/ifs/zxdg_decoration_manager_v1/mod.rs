use crate::client::Client;
use crate::globals::{Global, GlobalName};
use crate::ifs::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;
use crate::object::{Interface, Object};
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
        let obj = Rc::new(ZxdgDecorationManagerV1 {
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

simple_add_global!(ZxdgDecorationManagerV1Global);

pub struct ZxdgDecorationManagerV1 {
    id: ZxdgDecorationManagerV1Id,
    client: Rc<Client>,
    _version: u32,
}

impl ZxdgDecorationManagerV1 {
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
        let tl = self.client.lookup(req.toplevel)?;
        let obj = Rc::new(ZxdgToplevelDecorationV1::new(req.id, &self.client, &tl));
        self.client.add_client_obj(&obj)?;
        obj.send_configure();
        Ok(())
    }
}

object_base! {
    ZxdgDecorationManagerV1, ZxdgDecorationManagerV1Error;

    DESTROY => destroy,
    GET_TOPLEVEL_DECORATION => get_toplevel_decoration,
}

impl Object for ZxdgDecorationManagerV1 {
    fn num_requests(&self) -> u32 {
        GET_TOPLEVEL_DECORATION + 1
    }
}

simple_add_obj!(ZxdgDecorationManagerV1);

use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZxdgDecorationManagerV1Id;
use crate::wire::zxdg_decoration_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
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
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ZxdgDecorationManagerV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for ZxdgDecorationManagerV1Global {
    fn version(&self) -> u32 {
        2
    }
}

#[derive(Object)]
pub struct ZxdgDecorationManagerV1 {
    id: ZxdgDecorationManagerV1Id,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
}

impl ZxdgDecorationManagerV1RequestHandler for ZxdgDecorationManagerV1 {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_toplevel_decoration(
        &self,
        req: GetToplevelDecoration,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let tl = self.client.lookup(req.toplevel)?;
        let obj = Rc::new(ZxdgToplevelDecorationV1::new(
            req.id,
            &self.client,
            &tl,
            self.version,
        ));
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        obj.do_send_configure();
        Ok(())
    }
}

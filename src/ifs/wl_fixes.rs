use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WlFixesId;
use crate::wire::wl_fixes::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

pub struct WlFixesGlobal {
    name: GlobalName,
}

impl WlFixesGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlFixesId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let mgr = Rc::new(WlFixes {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr);
        Ok(())
    }
}

global_base!(WlFixesGlobal, WlFixes);

simple_add_global!(WlFixesGlobal);

impl Global for WlFixesGlobal {
    fn version(&self) -> u32 {
        2
    }
}

#[derive(Object)]
pub struct WlFixes {
    id: WlFixesId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl WlFixesRequestHandler for WlFixes {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn destroy_registry(&self, req: DestroyRegistry, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let registry = self.client.lookup(req.registry)?;
        self.client.remove_obj(&*registry);
        Ok(())
    }

    fn ack_global_remove(&self, _req: AckGlobalRemove, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

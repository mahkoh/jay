use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wp_security_context_v1::WpSecurityContextV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpSecurityContextManagerV1Id;
use crate::wire::wp_security_context_manager_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

pub struct WpSecurityContextManagerV1Global {
    name: GlobalName,
}

impl WpSecurityContextManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpSecurityContextManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(WpSecurityContextManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

global_base!(WpSecurityContextManagerV1Global, WpSecurityContextManagerV1);

impl Global for WpSecurityContextManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpSecurityContextManagerV1Global);

#[derive(Object)]
pub struct WpSecurityContextManagerV1 {
    id: WpSecurityContextManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl WpSecurityContextManagerV1RequestHandler for WpSecurityContextManagerV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_listener(&self, req: CreateListener, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(WpSecurityContextV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            listen_fd: req.listen_fd,
            close_fd: req.close_fd,
            sandbox_engine: Default::default(),
            app_id: Default::default(),
            instance_id: Default::default(),
            committed: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        Ok(())
    }
}

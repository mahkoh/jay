use crate::client::CAP_GAMMA_CONTROL_MANAGER;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::zwlr_gamma_control_v1::*;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZwlrGammaControlManagerV1Id;
use crate::wire::zwlr_gamma_control_manager_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

pub struct ZwlrGammaControlManagerV1Global {
    name: GlobalName,
}

impl ZwlrGammaControlManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrGammaControlManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ZwlrGammaControlManagerV1 {
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

global_base!(ZwlrGammaControlManagerV1Global, ZwlrGammaControlManagerV1);

simple_add_global!(ZwlrGammaControlManagerV1Global);

impl Global for ZwlrGammaControlManagerV1Global {
    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_GAMMA_CONTROL_MANAGER
    }
}

#[derive(Object)]
pub struct ZwlrGammaControlManagerV1 {
    id: ZwlrGammaControlManagerV1Id,
    pub client: Rc<Client>,
    tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwlrGammaControlManagerV1RequestHandler for ZwlrGammaControlManagerV1 {
    type Error = LookupError;

    fn get_gamma_control(&self, req: GetGammaControl, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?.global.clone();
        let p = Rc::new(ZwlrGammaControlV1::new(req.id, slf, output.clone()));
        track!(self.client, p);
        self.client.add_client_obj(&p);
        let Some(size) = p.gamma_lut_size() else {
            p.send_failed();
            return Ok(());
        };
        let Some(node) = output.node() else {
            p.send_failed();
            return Ok(());
        };
        if node.active_zwlr_gamma_control.is_some() {
            p.send_failed();
            return Ok(());
        }
        p.send_gamma_size(size);
        node.active_zwlr_gamma_control.set(Some(p));
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

use {
    crate::{
        client::{CAP_GAMMA_CONTROL_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::zwlr_gamma_control_v1::*,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwlrGammaControlManagerV1Id, zwlr_gamma_control_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

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
    ) -> Result<(), ZwlrGammaControlManagerV1Error> {
        let obj = Rc::new(ZwlrGammaControlManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    ZwlrGammaControlManagerV1Global,
    ZwlrGammaControlManagerV1,
    ZwlrGammaControlManagerV1Error
);

simple_add_global!(ZwlrGammaControlManagerV1Global);

impl Global for ZwlrGammaControlManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_GAMMA_CONTROL_MANAGER
    }
}

pub struct ZwlrGammaControlManagerV1 {
    pub id: ZwlrGammaControlManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwlrGammaControlManagerV1RequestHandler for ZwlrGammaControlManagerV1 {
    type Error = ZwlrGammaControlManagerV1Error;

    fn get_gamma_control(&self, req: GetGammaControl, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?.global.clone();
        let p = Rc::new(ZwlrGammaControlV1::new(req.id, slf, output.clone()));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
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
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrGammaControlManagerV1;
    version = self.version;
}

impl Object for ZwlrGammaControlManagerV1 {}

simple_add_obj!(ZwlrGammaControlManagerV1);

#[derive(Debug, Error)]
pub enum ZwlrGammaControlManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrGammaControlManagerV1Error, ClientError);

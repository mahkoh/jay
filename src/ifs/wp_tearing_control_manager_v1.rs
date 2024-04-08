use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_tearing_control_v1::{WpTearingControlV1, WpTearingControlV1Error},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_tearing_control_manager_v1::*, WpTearingControlManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpTearingControlManagerV1Global {
    name: GlobalName,
}

impl WpTearingControlManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpTearingControlManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpTearingControlManagerV1Error> {
        let obj = Rc::new(WpTearingControlManagerV1 {
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
    WpTearingControlManagerV1Global,
    WpTearingControlManagerV1,
    WpTearingControlManagerV1Error
);

impl Global for WpTearingControlManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpTearingControlManagerV1Global);

pub struct WpTearingControlManagerV1 {
    pub id: WpTearingControlManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

object_base! {
    self = WpTearingControlManagerV1;
    version = self.version;
}

impl WpTearingControlManagerV1RequestHandler for WpTearingControlManagerV1 {
    type Error = WpTearingControlManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_tearing_control(
        &self,
        req: GetTearingControl,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let control = Rc::new(WpTearingControlV1 {
            id: req.id,
            surface,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, control);
        self.client.add_client_obj(&control)?;
        control.install()?;
        Ok(())
    }
}

impl Object for WpTearingControlManagerV1 {}

simple_add_obj!(WpTearingControlManagerV1);

#[derive(Debug, Error)]
pub enum WpTearingControlManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpTearingControlV1Error(#[from] WpTearingControlV1Error),
}
efrom!(WpTearingControlManagerV1Error, ClientError);

use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_fractional_scale_v1::{WpFractionalScaleError, WpFractionalScaleV1},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_fractional_scale_manager_v1::*, WpFractionalScaleManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFractionalScaleManagerV1Global {
    pub name: GlobalName,
}

pub struct WpFractionalScaleManagerV1 {
    pub id: WpFractionalScaleManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpFractionalScaleManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpFractionalScaleManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpFractionalScaleManagerError> {
        let obj = Rc::new(WpFractionalScaleManagerV1 {
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
    WpFractionalScaleManagerV1Global,
    WpFractionalScaleManagerV1,
    WpFractionalScaleManagerError
);

impl Global for WpFractionalScaleManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpFractionalScaleManagerV1Global);

impl WpFractionalScaleManagerV1RequestHandler for WpFractionalScaleManagerV1 {
    type Error = WpFractionalScaleManagerError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_fractional_scale(
        &self,
        req: GetFractionalScale,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let fs = Rc::new(WpFractionalScaleV1::new(req.id, &surface, self.version));
        track!(self.client, fs);
        fs.install()?;
        self.client.add_client_obj(&fs)?;
        fs.send_preferred_scale();
        Ok(())
    }
}

object_base! {
    self = WpFractionalScaleManagerV1;
    version = self.version;
}

impl Object for WpFractionalScaleManagerV1 {}

simple_add_obj!(WpFractionalScaleManagerV1);

#[derive(Debug, Error)]
pub enum WpFractionalScaleManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpFractionalScaleError(#[from] WpFractionalScaleError),
}
efrom!(WpFractionalScaleManagerError, ClientError);

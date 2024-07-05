use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_alpha_modifier_surface_v1::{
            WpAlphaModifierSurfaceV1, WpAlphaModifierSurfaceV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_alpha_modifier_v1::*, WpAlphaModifierV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpAlphaModifierV1Global {
    name: GlobalName,
}

pub struct WpAlphaModifierV1 {
    id: WpAlphaModifierV1Id,
    client: Rc<Client>,
    version: Version,
    pub tracker: Tracker<Self>,
}

impl WpAlphaModifierV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpAlphaModifierV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpAlphaModifierV1Error> {
        let obj = Rc::new(WpAlphaModifierV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}
impl WpAlphaModifierV1RequestHandler for WpAlphaModifierV1 {
    type Error = WpAlphaModifierV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_surface(&self, req: GetSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let modifier = Rc::new(WpAlphaModifierSurfaceV1::new(
            req.id,
            &surface,
            self.version,
        ));
        track!(self.client, modifier);
        self.client.add_client_obj(&modifier)?;
        modifier.install()?;
        Ok(())
    }
}

global_base!(
    WpAlphaModifierV1Global,
    WpAlphaModifierV1,
    WpAlphaModifierV1Error
);

impl Global for WpAlphaModifierV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpAlphaModifierV1Global);

object_base! {
    self = WpAlphaModifierV1;
    version = self.version;
}

impl Object for WpAlphaModifierV1 {}

simple_add_obj!(WpAlphaModifierV1);

#[derive(Debug, Error)]
pub enum WpAlphaModifierV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpAlphaModifierSurfaceV1Error(#[from] WpAlphaModifierSurfaceV1Error),
}

efrom!(WpAlphaModifierV1Error, ClientError);

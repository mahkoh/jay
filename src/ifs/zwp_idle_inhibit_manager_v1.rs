use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_surface::zwp_idle_inhibitor_v1::{ZwpIdleInhibitorV1, ZwpIdleInhibitorV1Error},
            zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_idle_inhibit_manager_v1::*, ZwpIdleInhibitManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpIdleInhibitManagerV1Global {
    name: GlobalName,
}

impl ZwpIdleInhibitManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpIdleInhibitManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZxdgDecorationManagerV1Error> {
        let obj = Rc::new(ZwpIdleInhibitManagerV1 {
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

global_base!(
    ZwpIdleInhibitManagerV1Global,
    ZwpIdleInhibitManagerV1,
    ZwpIdleInhibitManagerV1Error
);

impl Global for ZwpIdleInhibitManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpIdleInhibitManagerV1Global);

pub struct ZwpIdleInhibitManagerV1 {
    pub id: ZwpIdleInhibitManagerV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl ZwpIdleInhibitManagerV1RequestHandler for ZwpIdleInhibitManagerV1 {
    type Error = ZwpIdleInhibitManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_inhibitor(&self, req: CreateInhibitor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let inhibit = Rc::new(ZwpIdleInhibitorV1 {
            id: req.id,
            inhibit_id: self.client.state.idle_inhibitor_ids.next(),
            client: self.client.clone(),
            surface,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, inhibit);
        self.client.add_client_obj(&inhibit)?;
        inhibit.install()?;
        Ok(())
    }
}

object_base! {
    self = ZwpIdleInhibitManagerV1;
    version = self.version;
}

impl Object for ZwpIdleInhibitManagerV1 {}

simple_add_obj!(ZwpIdleInhibitManagerV1);

#[derive(Debug, Error)]
pub enum ZwpIdleInhibitManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwpIdleInhibitorV1Error(#[from] ZwpIdleInhibitorV1Error),
}
efrom!(ZwpIdleInhibitManagerV1Error, ClientError);

use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_fifo_v1::{WpFifoV1, WpFifoV1Error},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_fifo_manager_v1::*, WpFifoManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFifoManagerV1Global {
    pub name: GlobalName,
}

pub struct WpFifoManagerV1 {
    pub id: WpFifoManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpFifoManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpFifoManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpFifoManagerV1Error> {
        let obj = Rc::new(WpFifoManagerV1 {
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

global_base!(WpFifoManagerV1Global, WpFifoManagerV1, WpFifoManagerV1Error);

impl Global for WpFifoManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpFifoManagerV1Global);

impl WpFifoManagerV1RequestHandler for WpFifoManagerV1 {
    type Error = WpFifoManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_fifo(&self, req: GetFifo, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let fs = Rc::new(WpFifoV1::new(req.id, self.version, &surface));
        track!(self.client, fs);
        fs.install()?;
        self.client.add_client_obj(&fs)?;
        Ok(())
    }
}

object_base! {
    self = WpFifoManagerV1;
    version = self.version;
}

impl Object for WpFifoManagerV1 {}

simple_add_obj!(WpFifoManagerV1);

#[derive(Debug, Error)]
pub enum WpFifoManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpFifoV1Error(#[from] WpFifoV1Error),
}
efrom!(WpFifoManagerV1Error, ClientError);

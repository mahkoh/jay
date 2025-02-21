use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_viewport::{WpViewport, WpViewportError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WpViewporterId, wp_viewporter::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpViewporterGlobal {
    pub name: GlobalName,
}

impl WpViewporterGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpViewporterId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpViewporterError> {
        let obj = Rc::new(WpViewporter {
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

global_base!(WpViewporterGlobal, WpViewporter, WpViewporterError);

impl Global for WpViewporterGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpViewporterGlobal);

pub struct WpViewporter {
    pub id: WpViewporterId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpViewporterRequestHandler for WpViewporter {
    type Error = WpViewporterError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_viewport(&self, req: GetViewport, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let viewport = Rc::new(WpViewport::new(req.id, &surface, self.version));
        track!(self.client, viewport);
        viewport.install()?;
        self.client.add_client_obj(&viewport)?;
        Ok(())
    }
}

object_base! {
    self = WpViewporter;
    version = self.version;
}

impl Object for WpViewporter {}

simple_add_obj!(WpViewporter);

#[derive(Debug, Error)]
pub enum WpViewporterError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpViewportError(#[from] WpViewportError),
}
efrom!(WpViewporterError, ClientError);

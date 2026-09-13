use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::wp_viewport::WpViewport;
use crate::ifs::wl_surface::wp_viewport::WpViewportError;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpViewporterId;
use crate::wire::wp_viewporter::*;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Global)]
pub struct WpViewporterGlobal {
    name: GlobalName,
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
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for WpViewporterGlobal {
    fn version(&self) -> u32 {
        1
    }
}

#[derive(Object)]
pub struct WpViewporter {
    id: WpViewporterId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl WpViewporterRequestHandler for WpViewporter {
    type Error = WpViewporterError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_viewport(&self, req: GetViewport, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let viewport = Rc::new(WpViewport::new(req.id, &surface, self.version));
        track!(self.client, viewport);
        viewport.install()?;
        self.client.add_client_obj(&viewport);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WpViewporterError {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error(transparent)]
    WpViewportError(#[from] WpViewportError),
}

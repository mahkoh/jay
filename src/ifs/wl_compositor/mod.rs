mod types;

use crate::client::Client;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_region::WlRegion;
use crate::ifs::wl_surface::WlSurface;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const CREATE_SURFACE: u32 = 0;
const CREATE_REGION: u32 = 1;

id!(WlCompositorId);

pub struct WlCompositorGlobal {
    name: GlobalName,
}

pub struct WlCompositor {
    id: WlCompositorId,
    client: Rc<Client>,
    _version: u32,
}

impl WlCompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlCompositorId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlCompositorError> {
        let obj = Rc::new(WlCompositor {
            id,
            client: client.clone(),
            _version: version,
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlCompositor {
    fn create_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateSurfaceError> {
        let surface: CreateSurface = self.client.parse(self, parser)?;
        let surface = Rc::new(WlSurface::new(surface.id, &self.client));
        self.client.add_client_obj(&surface)?;
        Ok(())
    }

    fn create_region(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateRegionError> {
        let region: CreateRegion = self.client.parse(self, parser)?;
        let region = Rc::new(WlRegion::new(region.id, &self.client));
        self.client.add_client_obj(&region)?;
        Ok(())
    }
}

global_base!(WlCompositorGlobal, WlCompositor, WlCompositorError);

impl Global for WlCompositorGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        4
    }
}

simple_add_global!(WlCompositorGlobal);

object_base! {
    WlCompositor, WlCompositorError;

    CREATE_SURFACE => create_surface,
    CREATE_REGION => create_region,
}

impl Object for WlCompositor {
    fn num_requests(&self) -> u32 {
        CREATE_REGION + 1
    }
}

simple_add_obj!(WlCompositor);

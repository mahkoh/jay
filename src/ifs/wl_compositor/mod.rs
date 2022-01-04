mod types;

use crate::client::{AddObj, Client};
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_region::WlRegion;
use crate::ifs::wl_surface::WlSurface;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const CREATE_SURFACE: u32 = 0;
const CREATE_REGION: u32 = 1;

id!(WlCompositorId);

pub struct WlCompositorGlobal {
    name: GlobalName,
}

pub struct WlCompositorObj {
    global: Rc<WlCompositorGlobal>,
    id: WlCompositorId,
    client: Rc<Client>,
    version: u32,
}

impl WlCompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    async fn bind_(
        self: Rc<Self>,
        id: WlCompositorId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlCompositorError> {
        let obj = Rc::new(WlCompositorObj {
            global: self,
            id,
            client: client.clone(),
            version,
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlCompositorObj {
    async fn create_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateSurfaceError> {
        let surface: CreateSurface = self.client.parse(self, parser)?;
        let surface = Rc::new(WlSurface::new(surface.id, &self.client));
        self.client.add_client_obj(&surface)?;
        Ok(())
    }

    async fn create_region(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateRegionError> {
        let region: CreateRegion = self.client.parse(self, parser)?;
        let region = Rc::new(WlRegion::new(region.id, &self.client));
        self.client.add_client_obj(&region)?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlCompositorError> {
        match request {
            CREATE_SURFACE => self.create_surface(parser).await?,
            CREATE_REGION => self.create_region(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

bind!(WlCompositorGlobal);

impl Global for WlCompositorGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::WlCompositor
    }

    fn version(&self) -> u32 {
        4
    }

    fn pre_remove(&self) {
        unreachable!()
    }
}

handle_request!(WlCompositorObj);

impl Object for WlCompositorObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlCompositor
    }

    fn num_requests(&self) -> u32 {
        CREATE_REGION + 1
    }
}

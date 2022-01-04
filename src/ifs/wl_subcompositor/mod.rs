mod types;

use crate::client::{AddObj, Client};
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_surface::wl_subsurface::WlSubsurface;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const GET_SUBSURFACE: u32 = 1;

const BAD_SURFACE: u32 = 0;

id!(WlSubcompositorId);

pub struct WlSubcompositorGlobal {
    name: GlobalName,
}

pub struct WlSubcompositorObj {
    global: Rc<WlSubcompositorGlobal>,
    id: WlSubcompositorId,
    client: Rc<Client>,
}

impl WlSubcompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    async fn bind_(
        self: Rc<Self>,
        id: WlSubcompositorId,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WlSubcompositorError> {
        let obj = Rc::new(WlSubcompositorObj {
            global: self,
            id,
            client: client.clone(),
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlSubcompositorObj {
    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn get_subsurface(&self, parser: MsgParser<'_, '_>) -> Result<(), GetSubsurfaceError> {
        let req: GetSubsurface = self.client.parse(self, parser)?;
        let surface = self.client.get_surface(req.surface)?;
        let parent = self.client.get_surface(req.parent)?;
        let subsurface = Rc::new(WlSubsurface::new(req.id, &surface, &parent));
        self.client.add_client_obj(&subsurface)?;
        subsurface.install()?;
        Ok(())
    }

    async fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSubcompositorError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            GET_SUBSURFACE => self.get_subsurface(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

bind!(WlSubcompositorGlobal);

impl Global for WlSubcompositorGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::WlSubcompositor
    }

    fn version(&self) -> u32 {
        1
    }

    fn pre_remove(&self) {
        unreachable!()
    }
}

handle_request!(WlSubcompositorObj);

impl Object for WlSubcompositorObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlSubcompositor
    }

    fn num_requests(&self) -> u32 {
        GET_SUBSURFACE + 1
    }
}

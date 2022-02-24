use crate::client::{Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_region::WlRegion;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_compositor::*;
use crate::wire::WlCompositorId;
use crate::xwayland::XWaylandEvent;
use std::rc::Rc;
use thiserror::Error;

pub struct WlCompositorGlobal {
    name: GlobalName,
}

pub struct WlCompositor {
    id: WlCompositorId,
    client: Rc<Client>,
    _version: u32,
    pub tracker: Tracker<Self>,
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
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlCompositor {
    fn create_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateSurfaceError> {
        let surface: CreateSurface = self.client.parse(self, parser)?;
        let surface = Rc::new(WlSurface::new(surface.id, &self.client));
        track!(self.client, surface);
        self.client.add_client_obj(&surface)?;
        if let Some(queue) = &self.client.xwayland_queue {
            queue.push(XWaylandEvent::SurfaceCreated(surface.clone()));
        }
        Ok(())
    }

    fn create_region(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateRegionError> {
        let region: CreateRegion = self.client.parse(self, parser)?;
        let region = Rc::new(WlRegion::new(region.id, &self.client));
        track!(self.client, region);
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

#[derive(Debug, Error)]
pub enum WlCompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `create_surface` request")]
    CreateSurfaceError(#[source] Box<CreateSurfaceError>),
    #[error("Could not process `create_region` request")]
    CreateRegionError(#[source] Box<CreateRegionError>),
}

efrom!(WlCompositorError, ClientError);
efrom!(WlCompositorError, CreateSurfaceError);
efrom!(WlCompositorError, CreateRegionError);

#[derive(Debug, Error)]
pub enum CreateSurfaceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(CreateSurfaceError, ParseFailed, MsgParserError);
efrom!(CreateSurfaceError, ClientError);

#[derive(Debug, Error)]
pub enum CreateRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(CreateRegionError, ParseFailed, MsgParserError);
efrom!(CreateRegionError, ClientError, ClientError);

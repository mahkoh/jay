use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{wl_region::WlRegion, wl_surface::WlSurface},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_compositor::*, WlCompositorId},
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlCompositorGlobal {
    name: GlobalName,
}

pub struct WlCompositor {
    id: WlCompositorId,
    client: Rc<Client>,
    version: u32,
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
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlCompositor {
    fn create_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), WlCompositorError> {
        let surface: CreateSurface = self.client.parse(self, parser)?;
        let surface = Rc::new(WlSurface::new(surface.id, &self.client, self.version));
        track!(self.client, surface);
        self.client.add_client_obj(&surface)?;
        if self.client.is_xwayland {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::SurfaceCreated(surface.id));
        }
        Ok(())
    }

    fn create_region(&self, parser: MsgParser<'_, '_>) -> Result<(), WlCompositorError> {
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
        5
    }
}

simple_add_global!(WlCompositorGlobal);

object_base! {
    WlCompositor;

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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}

efrom!(WlCompositorError, ClientError);
efrom!(WlCompositorError, MsgParserError);

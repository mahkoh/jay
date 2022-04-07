use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wl_subsurface::{WlSubsurface, WlSubsurfaceError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_subcompositor::*, WlSubcompositorId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[allow(dead_code)]
const BAD_SURFACE: u32 = 0;

pub struct WlSubcompositorGlobal {
    name: GlobalName,
}

pub struct WlSubcompositor {
    id: WlSubcompositorId,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WlSubcompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlSubcompositorId,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WlSubcompositorError> {
        let obj = Rc::new(WlSubcompositor {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlSubcompositor {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_subsurface(&self, parser: MsgParser<'_, '_>) -> Result<(), GetSubsurfaceError> {
        let req: GetSubsurface = self.client.parse(self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let parent = self.client.lookup(req.parent)?;
        let subsurface = Rc::new(WlSubsurface::new(req.id, &surface, &parent));
        track!(self.client, subsurface);
        self.client.add_client_obj(&subsurface)?;
        subsurface.install()?;
        Ok(())
    }
}

global_base!(WlSubcompositorGlobal, WlSubcompositor, WlSubcompositorError);

impl Global for WlSubcompositorGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WlSubcompositorGlobal);

object_base! {
    WlSubcompositor, WlSubcompositorError;

    DESTROY => destroy,
    GET_SUBSURFACE => get_subsurface,
}

impl Object for WlSubcompositor {
    fn num_requests(&self) -> u32 {
        GET_SUBSURFACE + 1
    }
}

simple_add_obj!(WlSubcompositor);

#[derive(Debug, Error)]
pub enum WlSubcompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `get_subsurface` request")]
    GetSubsurfaceError(#[from] GetSubsurfaceError),
}
efrom!(WlSubcompositorError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum GetSubsurfaceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    SubsurfaceError(Box<WlSubsurfaceError>),
}
efrom!(GetSubsurfaceError, ParseFailed, MsgParserError);
efrom!(GetSubsurfaceError, ClientError);
efrom!(GetSubsurfaceError, SubsurfaceError, WlSubsurfaceError);

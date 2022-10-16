use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::{x_surface::xwayland_surface_v1::XwaylandSurfaceV1, WlSurfaceError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{xwayland_shell_v1::*, WlSurfaceId, XwaylandShellV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct XwaylandShellV1Global {
    name: GlobalName,
}

pub struct XwaylandShellV1 {
    id: XwaylandShellV1Id,
    client: Rc<Client>,
    pub version: u32,
    pub tracker: Tracker<Self>,
}

impl XwaylandShellV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XwaylandShellV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), XwaylandShellV1Error> {
        let obj = Rc::new(XwaylandShellV1 {
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
impl XwaylandShellV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), XwaylandShellV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_xwayland_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), XwaylandShellV1Error> {
        let req: GetXwaylandSurface = self.client.parse(self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let xsurface = surface.get_xsurface()?;
        if xsurface.xwayland_surface.get().is_some() {
            return Err(XwaylandShellV1Error::AlreadyAttached(surface.id));
        }
        let xws = Rc::new(XwaylandSurfaceV1 {
            id: req.id,
            client: self.client.clone(),
            x: xsurface,
            tracker: Default::default(),
        });
        track!(self.client, xws);
        xws.x.xwayland_surface.set(Some(xws.clone()));
        self.client.add_client_obj(&xws)?;
        Ok(())
    }
}

global_base!(XwaylandShellV1Global, XwaylandShellV1, XwaylandShellV1Error);

impl Global for XwaylandShellV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn xwayland_only(&self) -> bool {
        true
    }
}

simple_add_global!(XwaylandShellV1Global);

object_base! {
    XwaylandShellV1;

    DESTROY => destroy,
    GET_XWAYLAND_SURFACE => get_xwayland_surface,
}

impl Object for XwaylandShellV1 {
    fn num_requests(&self) -> u32 {
        GET_XWAYLAND_SURFACE + 1
    }
}

simple_add_obj!(XwaylandShellV1);

#[derive(Debug, Error)]
pub enum XwaylandShellV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("The `wl_surface` {0} already has an extension object")]
    AlreadyAttached(WlSurfaceId),
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
}
efrom!(XwaylandShellV1Error, ClientError);
efrom!(XwaylandShellV1Error, MsgParserError);

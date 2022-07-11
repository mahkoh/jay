use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError},
            xdg_positioner::XdgPositioner,
        },
        leaks::Tracker,
        object::Object,
        utils::{
            buffd::{MsgParser, MsgParserError},
            copyhashmap::CopyHashMap,
        },
        wire::{xdg_wm_base::*, XdgSurfaceId, XdgWmBaseId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[allow(dead_code)]
const ROLE: u32 = 0;
const DEFUNCT_SURFACES: u32 = 1;
#[allow(dead_code)]
const NOT_THE_TOPMOST_POPUP: u32 = 2;
#[allow(dead_code)]
const INVALID_POPUP_PARENT: u32 = 3;
#[allow(dead_code)]
const INVALID_SURFACE_STATE: u32 = 4;
#[allow(dead_code)]
const INVALID_POSITIONER: u32 = 5;

pub struct XdgWmBaseGlobal {
    name: GlobalName,
}

pub struct XdgWmBase {
    id: XdgWmBaseId,
    client: Rc<Client>,
    pub version: u32,
    pub(super) surfaces: CopyHashMap<XdgSurfaceId, Rc<XdgSurface>>,
    pub tracker: Tracker<Self>,
}

impl XdgWmBaseGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgWmBaseId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), XdgWmBaseError> {
        let obj = Rc::new(XdgWmBase {
            id,
            client: client.clone(),
            version,
            surfaces: Default::default(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl XdgWmBase {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgWmBaseError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        if !self.surfaces.is_empty() {
            self.client.protocol_error(
                self,
                DEFUNCT_SURFACES,
                &format!(
                    "Cannot destroy xdg_wm_base object {} before destroying its surfaces",
                    self.id
                ),
            );
            return Err(XdgWmBaseError::DefunctSurfaces);
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_positioner(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgWmBaseError> {
        let req: CreatePositioner = self.client.parse(&**self, parser)?;
        let pos = Rc::new(XdgPositioner::new(self, req.id, &self.client));
        track!(self.client, pos);
        self.client.add_client_obj(&pos)?;
        Ok(())
    }

    fn get_xdg_surface(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgWmBaseError> {
        let req: GetXdgSurface = self.client.parse(&**self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let xdg_surface = Rc::new(XdgSurface::new(self, req.id, &surface));
        track!(self.client, xdg_surface);
        self.client.add_client_obj(&xdg_surface)?;
        xdg_surface.install()?;
        self.surfaces.set(req.id, xdg_surface);
        Ok(())
    }

    fn pong(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgWmBaseError> {
        let _req: Pong = self.client.parse(self, parser)?;
        Ok(())
    }
}

global_base!(XdgWmBaseGlobal, XdgWmBase, XdgWmBaseError);

impl Global for XdgWmBaseGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        5
    }
}

simple_add_global!(XdgWmBaseGlobal);

object_base! {
    XdgWmBase;

    DESTROY => destroy,
    CREATE_POSITIONER => create_positioner,
    GET_XDG_SURFACE => get_xdg_surface,
    PONG => pong,
}

dedicated_add_obj!(XdgWmBase, XdgWmBaseId, xdg_wm_bases);

impl Object for XdgWmBase {
    fn num_requests(&self) -> u32 {
        PONG + 1
    }

    fn break_loops(&self) {
        self.surfaces.clear();
    }
}

#[derive(Debug, Error)]
pub enum XdgWmBaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("Tried to destroy xdg_wm_base object before destroying its surfaces")]
    DefunctSurfaces,
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
}
efrom!(XdgWmBaseError, ClientError);
efrom!(XdgWmBaseError, MsgParserError);
efrom!(XdgWmBaseError, XdgSurfaceError);

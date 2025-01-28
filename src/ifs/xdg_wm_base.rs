use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError},
            xdg_positioner::XdgPositioner,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::copyhashmap::CopyHashMap,
        wire::{xdg_wm_base::*, XdgSurfaceId, XdgWmBaseId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[expect(dead_code)]
const ROLE: u32 = 0;
const DEFUNCT_SURFACES: u32 = 1;
#[expect(dead_code)]
const NOT_THE_TOPMOST_POPUP: u32 = 2;
#[expect(dead_code)]
const INVALID_POPUP_PARENT: u32 = 3;
#[expect(dead_code)]
const INVALID_SURFACE_STATE: u32 = 4;
#[expect(dead_code)]
const INVALID_POSITIONER: u32 = 5;

pub struct XdgWmBaseGlobal {
    name: GlobalName,
}

pub struct XdgWmBase {
    id: XdgWmBaseId,
    client: Rc<Client>,
    pub version: Version,
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
        version: Version,
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

impl XdgWmBaseRequestHandler for XdgWmBase {
    type Error = XdgWmBaseError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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

    fn create_positioner(&self, req: CreatePositioner, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pos = Rc::new(XdgPositioner::new(slf, req.id, &self.client));
        track!(self.client, pos);
        self.client.add_client_obj(&pos)?;
        Ok(())
    }

    fn get_xdg_surface(&self, req: GetXdgSurface, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let xdg_surface = Rc::new(XdgSurface::new(slf, req.id, &surface));
        track!(self.client, xdg_surface);
        self.client.add_client_obj(&xdg_surface)?;
        xdg_surface.install()?;
        self.surfaces.set(req.id, xdg_surface);
        Ok(())
    }

    fn pong(&self, _req: Pong, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

global_base!(XdgWmBaseGlobal, XdgWmBase, XdgWmBaseError);

impl Global for XdgWmBaseGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        7
    }
}

simple_add_global!(XdgWmBaseGlobal);

object_base! {
    self = XdgWmBase;
    version = self.version;
}

dedicated_add_obj!(XdgWmBase, XdgWmBaseId, xdg_wm_bases);

impl Object for XdgWmBase {
    fn break_loops(&self) {
        self.surfaces.clear();
    }
}

#[derive(Debug, Error)]
pub enum XdgWmBaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Tried to destroy xdg_wm_base object before destroying its surfaces")]
    DefunctSurfaces,
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
}
efrom!(XdgWmBaseError, ClientError);
efrom!(XdgWmBaseError, XdgSurfaceError);

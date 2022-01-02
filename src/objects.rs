use crate::ifs::wl_compositor::WlCompositorError;
use crate::ifs::wl_display::{WlDisplay, WlDisplayError};
use crate::ifs::wl_registry::{WlRegistry, WlRegistryError};
use crate::ifs::wl_shm::WlShmError;
use crate::ifs::wl_shm_pool::WlShmPoolError;
use crate::ifs::wl_surface::{WlSurface, WlSurfaceError};
use crate::utils::buffd::{WlParser, WlParserError};
use crate::utils::copyhashmap::CopyHashMap;
use crate::wl_client::{WlClientData, WlClientError};
use ahash::AHashMap;
use std::cell::{RefCell, RefMut};
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::wl_region::{WlRegion, WlRegionError};

#[derive(Debug, Error)]
pub enum ObjectError {
    #[error("A client error occurred")]
    ClientError(#[source] Box<WlClientError>),
    #[error("Cannot parse the message")]
    ParserError(#[source] Box<WlParserError>),
    #[error("Server tried to allocate more than 0x1_00_00_00 ids")]
    TooManyIds,
    #[error("The server object id is out of bounds")]
    ServerIdOutOfBounds,
    #[error("The object id is unknown")]
    UnknownId,
    #[error("The id is already in use")]
    IdAlreadyInUse,
    #[error("The client object id is out of bounds")]
    ClientIdOutOfBounds,
    #[error("An error occurred in a `wl_display`")]
    WlDisplayError(#[source] Box<WlDisplayError>),
    #[error("An error occurred in a `wl_registry`")]
    WlRegistryError(#[source] Box<WlRegistryError>),
    #[error("Could not add object {0} to the client")]
    AddObjectError(ObjectId, #[source] Box<ObjectError>),
    #[error("An error occurred in a `wl_surface`")]
    WlSurfaceError(#[source] Box<WlSurfaceError>),
    #[error("An error occurred in a `wl_compositor`")]
    WlCompositorError(#[source] Box<WlCompositorError>),
    #[error("An error occurred in a `wl_shm`")]
    WlShmError(#[source] Box<WlShmError>),
    #[error("An error occurred in a `wl_shm_pool`")]
    WlShmPoolError(#[source] Box<WlShmPoolError>),
    #[error("An error occurred in a `wl_region`")]
    WlRegionError(#[source] Box<WlRegionError>),
    #[error("Object {0} is not a display")]
    NotADisplay(ObjectId),
}

efrom!(ObjectError, ClientError, WlClientError);
efrom!(ObjectError, ParserError, WlParserError);
efrom!(ObjectError, WlDisplayError, WlDisplayError);
efrom!(ObjectError, WlRegistryError, WlRegistryError);
efrom!(ObjectError, WlSurfaceError, WlSurfaceError);
efrom!(ObjectError, WlCompositorError, WlCompositorError);
efrom!(ObjectError, WlShmError, WlShmError);
efrom!(ObjectError, WlShmPoolError, WlShmPoolError);
efrom!(ObjectError, WlRegionError, WlRegionError);

pub const WL_DISPLAY_ID: ObjectId = ObjectId(1);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(u32);

impl ObjectId {
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl Display for ObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub trait ObjectHandleRequest {
    fn handle_request<'a>(
        &'a self,
        request: u32,
        parser: WlParser<'a, 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<(), ObjectError>> + 'a>>;
}

pub trait Object: ObjectHandleRequest {
    fn id(&self) -> ObjectId;
    fn interface(&self) -> Interface;
    fn num_requests(&self) -> u32;
    fn pre_release(&self) -> Result<(), ObjectError> {
        Ok(())
    }
    fn post_attach(self: Rc<Self>) {}
    fn into_display(self: Rc<Self>) -> Result<Rc<WlDisplay>, ObjectError> {
        Err(ObjectError::NotADisplay(self.id()))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Interface {
    WlDisplay,
    WlCallback,
    WlCompositor,
    WlRegistry,
    WlShm,
    WlShmPool,
    WlSubcompositor,
    XdgWmBase,
    WlSurface,
    WlRegion,
}

impl Interface {
    pub fn name(self) -> &'static str {
        match self {
            Interface::WlDisplay => "wl_display",
            Interface::WlCallback => "wl_callback",
            Interface::WlCompositor => "wl_compositor",
            Interface::WlRegistry => "wl_registry",
            Interface::WlShm => "wl_shm",
            Interface::WlSubcompositor => "wl_subcompositor",
            Interface::XdgWmBase => "xdg_wm_base",
            Interface::WlSurface => "wl_surface",
            Interface::WlShmPool => "wl_shm_pool",
            Interface::WlRegion => "wl_region",
        }
    }
}

pub struct Objects {
    registry: CopyHashMap<ObjectId, Rc<dyn Object>>,
    registries: CopyHashMap<ObjectId, Rc<WlRegistry>>,
    pub surfaces: CopyHashMap<ObjectId, Rc<WlSurface>>,
    pub regions: CopyHashMap<ObjectId, Rc<WlRegion>>,
    ids: RefCell<Vec<usize>>,
}

const MIN_SERVER_ID: u32 = 0xff000000;
const SEG_SIZE: usize = 8 * mem::size_of::<usize>();

impl Objects {
    pub fn new() -> Self {
        Self {
            registry: Default::default(),
            registries: Default::default(),
            surfaces: Default::default(),
            regions: Default::default(),
            ids: RefCell::new(vec![]),
        }
    }

    pub fn destroy(&self) {
        self.registry.clear();
        self.registries.clear();
        self.surfaces.clear();
    }

    fn id(&self, client_data: &WlClientData) -> Result<ObjectId, ObjectError> {
        const MAX_ID_OFFSET: u32 = u32::MAX - MIN_SERVER_ID;
        let offset = self.id_offset();
        if offset > MAX_ID_OFFSET {
            log::error!(
                "Client {} caused the server to allocate more than 0x{:x} ids",
                client_data.id,
                MAX_ID_OFFSET + 1
            );
            return Err(ObjectError::TooManyIds);
        }
        Ok(ObjectId(MIN_SERVER_ID + offset))
    }

    pub fn get_obj(&self, id: ObjectId) -> Result<Rc<dyn Object>, ObjectError> {
        match self.registry.get(&id) {
            Some(o) => Ok(o),
            _ => Err(ObjectError::UnknownId),
        }
    }

    pub fn add_client_object(&self, obj: Rc<dyn Object>) -> Result<(), ObjectError> {
        let id = obj.id();
        let res = (|| {
            if id.0 == 0 || id.0 >= MIN_SERVER_ID {
                return Err(ObjectError::ClientIdOutOfBounds);
            }
            if self.registry.contains(&id) {
                return Err(ObjectError::IdAlreadyInUse);
            }
            self.registry.set(id, obj.clone());
            obj.post_attach();
            Ok(())
        })();
        if let Err(e) = res {
            return Err(ObjectError::AddObjectError(id, Box::new(e)));
        }
        Ok(())
    }

    pub async fn remove_obj(
        &self,
        client_data: &WlClientData,
        id: ObjectId,
    ) -> Result<(), ObjectError> {
        let obj = match self.registry.remove(&id) {
            Some(o) => o,
            _ => return Err(ObjectError::UnknownId),
        };
        obj.pre_release()?;
        if id.0 >= MIN_SERVER_ID {
            let offset = (id.0 - MIN_SERVER_ID) as usize;
            let pos = offset / SEG_SIZE;
            let seg_offset = offset % SEG_SIZE;
            let mut ids = self.ids.borrow_mut();
            if ids.len() <= pos {
                return Err(ObjectError::ServerIdOutOfBounds);
            }
            ids[pos] |= 1 << seg_offset;
        }
        client_data
            .event(client_data.display()?.delete_id(id))
            .await?;
        Ok(())
    }

    pub fn registries(&self) -> RefMut<AHashMap<ObjectId, Rc<WlRegistry>>> {
        self.registries.lock()
    }

    fn id_offset(&self) -> u32 {
        let mut ids = self.ids.borrow_mut();
        for (pos, seg) in ids.iter_mut().enumerate() {
            if *seg != 0 {
                let offset = seg.trailing_zeros();
                *seg &= !(1 << offset);
                return (pos * SEG_SIZE) as u32 + offset;
            }
        }
        ids.push(!1);
        ((ids.len() - 1) * SEG_SIZE) as u32
    }
}

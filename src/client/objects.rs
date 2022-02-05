use crate::client::{Client, ClientError};
use crate::ifs::wl_buffer::{WlBuffer, WlBufferId};
use crate::ifs::wl_data_source::{WlDataSource, WlDataSourceId};
use crate::ifs::wl_display::WlDisplay;
use crate::ifs::wl_region::{WlRegion, WlRegionId};
use crate::ifs::wl_registry::{WlRegistry, WlRegistryId};
use crate::ifs::wl_seat::{WlSeat, WlSeatId};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{XdgToplevel, XdgToplevelId};
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceId};
use crate::ifs::wl_surface::{WlSurface, WlSurfaceId};
use crate::ifs::xdg_positioner::{XdgPositioner, XdgPositionerId};
use crate::ifs::xdg_wm_base::{XdgWmBase, XdgWmBaseId};
use crate::ifs::zwp_primary_selection_source_v1::{
    ZwpPrimarySelectionSourceV1, ZwpPrimarySelectionSourceV1Id,
};
use crate::object::{Object, ObjectId};
use crate::tree::Node;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use ahash::AHashMap;
use std::cell::{RefCell, RefMut};
use std::mem;
use std::rc::Rc;

pub struct Objects {
    pub display: CloneCell<Option<Rc<WlDisplay>>>,
    registry: CopyHashMap<ObjectId, Rc<dyn Object>>,
    registries: CopyHashMap<WlRegistryId, Rc<WlRegistry>>,
    pub surfaces: CopyHashMap<WlSurfaceId, Rc<WlSurface>>,
    pub xdg_surfaces: CopyHashMap<XdgSurfaceId, Rc<XdgSurface>>,
    pub xdg_toplevel: CopyHashMap<XdgToplevelId, Rc<XdgToplevel>>,
    pub wl_data_source: CopyHashMap<WlDataSourceId, Rc<WlDataSource>>,
    pub zwp_primary_selection_source:
        CopyHashMap<ZwpPrimarySelectionSourceV1Id, Rc<ZwpPrimarySelectionSourceV1>>,
    pub xdg_positioners: CopyHashMap<XdgPositionerId, Rc<XdgPositioner>>,
    pub regions: CopyHashMap<WlRegionId, Rc<WlRegion>>,
    pub buffers: CopyHashMap<WlBufferId, Rc<WlBuffer>>,
    pub xdg_wm_bases: CopyHashMap<XdgWmBaseId, Rc<XdgWmBase>>,
    pub seats: CopyHashMap<WlSeatId, Rc<WlSeat>>,
    ids: RefCell<Vec<usize>>,
}

pub const MIN_SERVER_ID: u32 = 0xff000000;
const SEG_SIZE: usize = 8 * mem::size_of::<usize>();

impl Objects {
    pub fn new() -> Self {
        Self {
            display: CloneCell::new(None),
            registry: Default::default(),
            registries: Default::default(),
            surfaces: Default::default(),
            xdg_surfaces: Default::default(),
            xdg_toplevel: Default::default(),
            wl_data_source: Default::default(),
            zwp_primary_selection_source: Default::default(),
            xdg_positioners: Default::default(),
            regions: Default::default(),
            buffers: Default::default(),
            xdg_wm_bases: Default::default(),
            seats: Default::default(),
            ids: RefCell::new(vec![]),
        }
    }

    pub fn destroy(&self) {
        {
            let mut toplevel = self.xdg_toplevel.lock();
            for obj in toplevel.values_mut() {
                obj.destroy_node(true);
            }
            toplevel.clear();
        }
        {
            let mut registry = self.registry.lock();
            for obj in registry.values_mut() {
                obj.break_loops();
            }
            registry.clear();
        }
        self.display.set(None);
        self.registries.clear();
        self.surfaces.clear();
        self.xdg_surfaces.clear();
        self.wl_data_source.clear();
        self.zwp_primary_selection_source.clear();
        self.xdg_positioners.clear();
        self.regions.clear();
        self.buffers.clear();
        self.xdg_wm_bases.clear();
        self.seats.clear();
    }

    pub fn id<T>(&self, client_data: &Client) -> Result<T, ClientError>
    where
        ObjectId: Into<T>,
    {
        const MAX_ID_OFFSET: u32 = u32::MAX - MIN_SERVER_ID;
        let offset = self.id_offset();
        if offset > MAX_ID_OFFSET {
            log::error!(
                "Client {} caused the server to allocate more than 0x{:x} ids",
                client_data.id,
                MAX_ID_OFFSET + 1
            );
            return Err(ClientError::TooManyIds);
        }
        Ok(ObjectId::from_raw(MIN_SERVER_ID + offset).into())
    }

    pub fn get_obj(&self, id: ObjectId) -> Result<Rc<dyn Object>, ClientError> {
        match self.registry.get(&id) {
            Some(o) => Ok(o),
            _ => Err(ClientError::UnknownId),
        }
    }

    pub fn add_server_object(&self, obj: Rc<dyn Object>) {
        let id = obj.id();
        assert!(id.raw() >= MIN_SERVER_ID);
        assert!(!self.registry.contains(&id));
        self.registry.set(id, obj.clone());
    }

    pub fn add_client_object(&self, obj: Rc<dyn Object>) -> Result<(), ClientError> {
        let id = obj.id();
        let res = (|| {
            if id.raw() == 0 || id.raw() >= MIN_SERVER_ID {
                return Err(ClientError::ClientIdOutOfBounds);
            }
            if self.registry.contains(&id) {
                return Err(ClientError::IdAlreadyInUse);
            }
            self.registry.set(id, obj.clone());
            Ok(())
        })();
        if let Err(e) = res {
            return Err(ClientError::AddObjectError(id, Box::new(e)));
        }
        Ok(())
    }

    pub fn remove_obj(&self, client_data: &Rc<Client>, id: ObjectId) -> Result<(), ClientError> {
        let _obj = match self.registry.remove(&id) {
            Some(o) => o,
            _ => return Err(ClientError::UnknownId),
        };
        if id.raw() >= MIN_SERVER_ID {
            let offset = (id.raw() - MIN_SERVER_ID) as usize;
            let pos = offset / SEG_SIZE;
            let seg_offset = offset % SEG_SIZE;
            let mut ids = self.ids.borrow_mut();
            if ids.len() <= pos {
                return Err(ClientError::ServerIdOutOfBounds);
            }
            ids[pos] |= 1 << seg_offset;
        } else {
            client_data.event(client_data.display()?.delete_id(id));
        }
        Ok(())
    }

    pub fn registries(&self) -> RefMut<AHashMap<WlRegistryId, Rc<WlRegistry>>> {
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

use crate::client::{Client, ClientError};
use crate::ifs::wl_display::WlDisplay;
use crate::ifs::wl_region::WlRegion;
use crate::ifs::wl_registry::WlRegistry;
use crate::ifs::wl_surface::WlSurface;
use crate::object::{Object, ObjectId};
use crate::utils::copyhashmap::CopyHashMap;
use ahash::AHashMap;
use std::cell::{RefCell, RefMut};
use std::mem;
use std::rc::Rc;
use crate::ifs::xdg_wm_base::XdgWmBaseObj;

pub struct Objects {
    pub display: RefCell<Option<Rc<WlDisplay>>>,
    registry: CopyHashMap<ObjectId, Rc<dyn Object>>,
    registries: CopyHashMap<ObjectId, Rc<WlRegistry>>,
    pub surfaces: CopyHashMap<ObjectId, Rc<WlSurface>>,
    pub regions: CopyHashMap<ObjectId, Rc<WlRegion>>,
    pub xdg_wm_bases: CopyHashMap<ObjectId, Rc<XdgWmBaseObj>>,
    ids: RefCell<Vec<usize>>,
}

pub const MIN_SERVER_ID: u32 = 0xff000000;
const SEG_SIZE: usize = 8 * mem::size_of::<usize>();

impl Objects {
    pub fn new() -> Self {
        Self {
            display: RefCell::new(None),
            registry: Default::default(),
            registries: Default::default(),
            surfaces: Default::default(),
            regions: Default::default(),
            xdg_wm_bases: Default::default(),
            ids: RefCell::new(vec![]),
        }
    }

    pub fn destroy(&self) {
        {
            let mut surfaces = self.surfaces.lock();
            for surface in surfaces.values_mut() {
                surface.break_loops();
            }
        }
        {
            let mut xdg_wm_bases = self.xdg_wm_bases.lock();
            for xdg_wm_base in xdg_wm_bases.values_mut() {
                xdg_wm_base.break_loops();
            }
        }
        *self.display.borrow_mut() = None;
        self.registry.clear();
        self.regions.clear();
        self.registries.clear();
        self.surfaces.clear();
    }

    fn id(&self, client_data: &Client) -> Result<ObjectId, ClientError> {
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
        Ok(ObjectId::from_raw(MIN_SERVER_ID + offset))
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

    pub async fn remove_obj(&self, client_data: &Client, id: ObjectId) -> Result<(), ClientError> {
        if self.registry.remove(&id).is_none() {
            return Err(ClientError::UnknownId);
        }
        if id.raw() >= MIN_SERVER_ID {
            let offset = (id.raw() - MIN_SERVER_ID) as usize;
            let pos = offset / SEG_SIZE;
            let seg_offset = offset % SEG_SIZE;
            let mut ids = self.ids.borrow_mut();
            if ids.len() <= pos {
                return Err(ClientError::ServerIdOutOfBounds);
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

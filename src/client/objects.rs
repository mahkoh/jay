use crate::client::Client;
use crate::client::ClientError;
use crate::client::objects::dedicated::Dedicated;
use crate::ifs::wl_display::WlDisplay;
use crate::ifs::wl_registry::WlRegistry;
use crate::object::Object;
use crate::object::SyntheticObjectEventHandler;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::FCopyHashMap;
use crate::utils::copyhashmap::Locked;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::numcell::NumCell;
use crate::utils::reset_immutable::ResetImmutable;
use crate::utils::woid_hash::WoidBuildHasher;
use crate::utils::woid_hash::WoidCopyHashMap;
use crate::wire::ObjectId;
use crate::wire::WlRegistryId;
use derivative::Derivative;
use jay_proc::ResetImmutable;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(ResetImmutable, Derivative)]
#[derivative(Default)]
pub struct Objects {
    pub display: CloneCell<Option<Rc<WlDisplay>>>,
    registry: WoidCopyHashMap<ObjectId, Rc<dyn Object>>,
    synthetic_event_handlers: FCopyHashMap<ObjectId, Rc<dyn SyntheticObjectEventHandler>>,
    pub dedicated: Dedicated,
    ids: RefCell<Vec<usize>>,
    #[derivative(Default(value = "NumCell::new(FIRST_SYNTHETIC_ID)"))]
    next_synthetic_id: NumCell<u64>,
}

pub const MIN_SERVER_ID: u64 = 0xff000000;
pub const FIRST_SYNTHETIC_ID: u64 = u32::MAX as u64 + 1;
pub const FIRST_INVALID_ID: u64 = !0;
const SEG_SIZE: usize = usize::BITS as usize;

impl Objects {
    pub fn destroy(&self) {
        for surface in self.dedicated.wl_surface.lock().values() {
            if let Some(tl) = surface.get_toplevel() {
                tl.tl_destroy_dyn();
            }
        }
        for obj in self.registry.lock().drain_values() {
            obj.break_loops();
        }
        self.reset_immutable();
    }

    pub fn id(&self, client_data: &Client, parent: ObjectId) -> Result<ObjectId, ClientError> {
        if parent.raw() >= FIRST_SYNTHETIC_ID {
            return Ok(self.synthetic_id());
        }
        const MAX_ID_OFFSET: u64 = u32::MAX as u64 - MIN_SERVER_ID;
        let offset = self.id_offset() as u64;
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

    pub fn add_server_object(&self, id: ObjectId, obj: Rc<dyn Object>) {
        assert!(id.raw() >= MIN_SERVER_ID);
        assert!(!self.registry.contains(&id));
        self.registry.set(id, obj);
    }

    pub fn add_client_object(&self, id: ObjectId, obj: Rc<dyn Object>) -> Result<(), ClientError> {
        let raw = id.raw();
        let res = if raw == 0 || (raw >= MIN_SERVER_ID && raw < FIRST_SYNTHETIC_ID) {
            Err(ClientError::ClientIdOutOfBounds)
        } else if self.registry.contains(&id) {
            Err(ClientError::IdAlreadyInUse)
        } else {
            Ok(())
        };
        let mut registry_id = id;
        if res.is_err() {
            registry_id = self.synthetic_id();
        }
        self.registry.set(registry_id, obj);
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
        if id.raw() >= FIRST_SYNTHETIC_ID {
            return Ok(());
        }
        let mut send_delete = true;
        if id.raw() >= MIN_SERVER_ID {
            let offset = (id.raw() - MIN_SERVER_ID) as usize;
            let pos = offset / SEG_SIZE;
            let seg_offset = offset % SEG_SIZE;
            let mut ids = self.ids.borrow_mut();
            if ids.len() <= pos {
                return Err(ClientError::ServerIdOutOfBounds);
            }
            ids[pos] |= 1 << seg_offset;
            send_delete = client_data.symmetric_delete.get();
        }
        if send_delete {
            client_data.display()?.send_delete_id(id);
        }
        if client_data.tracers.num.get() > 0 {
            client_data.tracers.handle_delete_id(id);
        }
        Ok(())
    }

    pub fn registries(&self) -> Locked<'_, WlRegistryId, Rc<WlRegistry>, WoidBuildHasher> {
        self.dedicated.wl_registry.lock()
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

    pub fn synthetic_id(&self) -> ObjectId {
        ObjectId::from_raw(self.next_synthetic_id.fetch_add(1))
    }

    pub fn get_synthetic_event_handler(
        &self,
        id: ObjectId,
    ) -> Option<Rc<dyn SyntheticObjectEventHandler>> {
        self.synthetic_event_handlers.get(&id)
    }

    pub fn set_synthetic_event_handler(
        &self,
        id: ObjectId,
        event_handler: Rc<dyn SyntheticObjectEventHandler>,
    ) {
        assert!(id.raw() >= FIRST_SYNTHETIC_ID);
        self.synthetic_event_handlers.set(id, event_handler);
    }

    pub fn remove_synthetic_event_handler(&self, id: ObjectId) {
        self.synthetic_event_handlers.remove(&id);
    }
}

mod dedicated {
    include!(concat!(env!("OUT_DIR"), "/dedicated.rs"));
}

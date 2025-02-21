use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{WlRegistryId, wl_registry::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWlRegistry {
    pub id: WlRegistryId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlRegistryOwner>>>,
    pub version: Version,
}

pub trait UsrWlRegistryOwner {
    fn global(self: Rc<Self>, name: u32, interface: &str, version: u32) {
        let _ = name;
        let _ = interface;
        let _ = version;
    }

    fn global_remove(&self, name: u32) {
        let _ = name;
    }
}

impl UsrWlRegistry {
    pub fn request_bind(&self, name: u32, version: u32, obj: &dyn UsrObject) {
        self.con.request(Bind {
            self_id: self.id,
            name,
            interface: obj.interface().name(),
            version,
            id: obj.id(),
        });
    }
}

impl WlRegistryEventHandler for UsrWlRegistry {
    type Error = Infallible;

    fn global(&self, ev: Global<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.global(ev.name, ev.interface, ev.version);
        }
        Ok(())
    }

    fn global_remove(&self, ev: GlobalRemove, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.global_remove(ev.name);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlRegistry = WlRegistry;
    version = self.version;
}

impl UsrObject for UsrWlRegistry {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}

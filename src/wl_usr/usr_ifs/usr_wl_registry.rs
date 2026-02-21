use {
    crate::{
        globals::GlobalName,
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
    fn global(self: Rc<Self>, name: GlobalName, interface: &str, version: u32) {
        let _ = name;
        let _ = interface;
        let _ = version;
    }

    fn global_remove(&self, name: GlobalName) {
        let _ = name;
    }
}

impl UsrWlRegistry {
    pub fn bind(&self, name: GlobalName, obj: &dyn UsrObject) {
        self.con.request(Bind {
            self_id: self.id,
            name: name.raw(),
            interface: obj.interface().name(),
            version: obj.version().0,
            id: obj.id(),
        });
    }
}

impl WlRegistryEventHandler for UsrWlRegistry {
    type Error = Infallible;

    fn global(&self, ev: Global<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.global(GlobalName::from_raw(ev.name), ev.interface, ev.version);
        }
        Ok(())
    }

    fn global_remove(&self, ev: GlobalRemove, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.global_remove(GlobalName::from_raw(ev.name));
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

use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wl_registry::*, WlRegistryId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlRegistry {
    pub id: WlRegistryId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlRegistryOwner>>>,
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

    fn global(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Global = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.global(ev.name, ev.interface, ev.version);
        }
        Ok(())
    }

    fn global_remove(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: GlobalRemove = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.global_remove(ev.name);
        }
        Ok(())
    }
}

usr_object_base! {
    UsrWlRegistry, WlRegistry;

    GLOBAL => global,
    GLOBAL_REMOVE => global_remove,
}

impl UsrObject for UsrWlRegistry {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}

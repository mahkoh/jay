use {
    crate::{
        client::Client,
        globals::{Global, GlobalName, GlobalsError},
        leaks::Tracker,
        object::{Interface, Object, Version},
        wire::{WlRegistryId, wl_registry::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlRegistry {
    id: WlRegistryId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WlRegistry {
    pub fn new(id: WlRegistryId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
        }
    }

    pub fn send_global(self: &Rc<Self>, global: &Rc<dyn Global>) {
        self.client.event(crate::wire::wl_registry::Global {
            self_id: self.id,
            name: global.name().raw(),
            interface: global.interface().name(),
            version: global.version(),
        })
    }

    pub fn send_global_remove(self: &Rc<Self>, name: GlobalName) {
        self.client.event(GlobalRemove {
            self_id: self.id,
            name: name.raw(),
        })
    }
}

impl WlRegistryRequestHandler for WlRegistry {
    type Error = WlRegistryError;

    fn bind(&self, bind: Bind, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let name = GlobalName::from_raw(bind.name);
        let globals = &self.client.state.globals;
        let global = globals.get(name, self.client.effective_caps, self.client.is_xwayland)?;
        if global.interface().name() != bind.interface {
            return Err(WlRegistryError::InvalidInterface(InterfaceError {
                name: global.name(),
                interface: global.interface(),
                actual: bind.interface.to_string(),
            }));
        }
        if bind.version > global.version() {
            return Err(WlRegistryError::InvalidVersion(VersionError {
                name: global.name(),
                interface: global.interface(),
                version: global.version(),
                actual: bind.version,
            }));
        }
        global.bind(&self.client, bind.id, Version(bind.version))?;
        Ok(())
    }
}

object_base! {
    self = WlRegistry;
    version = Version(1);
}

impl Object for WlRegistry {}

dedicated_add_obj!(WlRegistry, WlRegistryId, registries);

#[derive(Debug, Error)]
pub enum WlRegistryError {
    #[error(transparent)]
    GlobalsError(Box<GlobalsError>),
    #[error("Tried to bind to global {} of type {} using interface {}", .0.name, .0.interface.name(), .0.actual)]
    InvalidInterface(InterfaceError),
    #[error("Tried to bind to global {} of type {} and version {} using version {}", .0.name, .0.interface.name(), .0.version, .0.actual)]
    InvalidVersion(VersionError),
}
efrom!(WlRegistryError, GlobalsError);

#[derive(Debug)]
pub struct InterfaceError {
    pub name: GlobalName,
    pub interface: Interface,
    pub actual: String,
}

#[derive(Debug)]
pub struct VersionError {
    pub name: GlobalName,
    pub interface: Interface,
    pub version: u32,
    pub actual: u32,
}

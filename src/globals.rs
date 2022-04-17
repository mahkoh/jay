use {
    crate::{
        backend::Backend,
        client::Client,
        ifs::{
            ipc::{
                wl_data_device_manager::WlDataDeviceManagerGlobal,
                zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1Global,
            },
            jay_compositor::JayCompositorGlobal,
            org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManagerGlobal,
            wl_compositor::WlCompositorGlobal,
            wl_drm::WlDrmGlobal,
            wl_output::WlOutputGlobal,
            wl_registry::WlRegistry,
            wl_seat::WlSeatGlobal,
            wl_shm::WlShmGlobal,
            wl_subcompositor::WlSubcompositorGlobal,
            xdg_wm_base::XdgWmBaseGlobal,
            zwlr_layer_shell_v1::ZwlrLayerShellV1Global,
            zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1Global,
            zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Global,
            zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Global,
            zxdg_output_manager_v1::ZxdgOutputManagerV1Global,
        },
        object::{Interface, ObjectId},
        state::State,
        utils::{
            copyhashmap::{CopyHashMap, Locked},
            numcell::NumCell,
        },
    },
    std::{
        error::Error,
        fmt::{Display, Formatter},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum GlobalsError {
    #[error("The requested global {0} does not exist")]
    GlobalDoesNotExist(GlobalName),
    #[error("The output with id {0} does not exist")]
    OutputDoesNotExist(GlobalName),
    #[error(transparent)]
    GlobalError(GlobalError),
}

#[derive(Debug, Error)]
#[error("An error occurred in a `{}` global", .interface.name())]
pub struct GlobalError {
    pub interface: Interface,
    #[source]
    pub error: Box<dyn Error>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct GlobalName(u32);

impl GlobalName {
    pub fn from_raw(id: u32) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl Display for GlobalName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub trait GlobalBase {
    fn name(&self) -> GlobalName;
    fn bind<'a>(
        self: Rc<Self>,
        client: &'a Rc<Client>,
        id: ObjectId,
        version: u32,
    ) -> Result<(), GlobalsError>;
    fn interface(&self) -> Interface;
}

pub trait Global: GlobalBase {
    fn singleton(&self) -> bool;
    fn version(&self) -> u32;
    fn break_loops(&self) {}
    fn secure(&self) -> bool {
        false
    }
}

pub struct Globals {
    next_name: NumCell<u32>,
    registry: CopyHashMap<GlobalName, Rc<dyn Global>>,
    pub outputs: CopyHashMap<GlobalName, Rc<WlOutputGlobal>>,
    pub seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
}

impl Globals {
    pub fn new() -> Self {
        Self {
            next_name: NumCell::new(1),
            registry: CopyHashMap::new(),
            outputs: Default::default(),
            seats: Default::default(),
        }
    }

    pub fn add_singletons(&self, backend: &Rc<dyn Backend>) {
        macro_rules! add_singleton {
            ($name:ident) => {
                self.add_global_no_broadcast(&Rc::new($name::new(self.name())));
            };
        }
        add_singleton!(WlCompositorGlobal);
        add_singleton!(WlShmGlobal);
        add_singleton!(WlSubcompositorGlobal);
        add_singleton!(XdgWmBaseGlobal);
        add_singleton!(WlDataDeviceManagerGlobal);
        add_singleton!(ZwpLinuxDmabufV1Global);
        add_singleton!(WlDrmGlobal);
        add_singleton!(ZxdgDecorationManagerV1Global);
        add_singleton!(OrgKdeKwinServerDecorationManagerGlobal);
        add_singleton!(ZwpPrimarySelectionDeviceManagerV1Global);
        add_singleton!(ZwlrLayerShellV1Global);
        add_singleton!(ZxdgOutputManagerV1Global);
        add_singleton!(JayCompositorGlobal);

        if backend.supports_idle() {
            add_singleton!(ZwpIdleInhibitManagerV1Global);
        }
    }

    pub fn name(&self) -> GlobalName {
        let id = self.next_name.fetch_add(1);
        if id == 0 {
            panic!("Global names overflowed");
        }
        GlobalName(id)
    }

    fn insert_no_broadcast<'a>(&'a self, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
    }

    fn insert_no_broadcast_<'a>(&'a self, global: &Rc<dyn Global>) {
        self.registry.set(global.name(), global.clone());
    }

    fn insert(&self, state: &State, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
        self.broadcast(state, global.secure(), |r| r.send_global(&global));
    }

    pub fn get(&self, name: GlobalName) -> Result<Rc<dyn Global>, GlobalsError> {
        self.take(name, false)
    }

    pub fn remove<T: WaylandGlobal>(&self, state: &State, global: &T) -> Result<(), GlobalsError> {
        let _global = self.take(global.name(), true)?;
        global.remove(self);
        self.broadcast(state, global.secure(), |r| {
            r.send_global_remove(global.name())
        });
        Ok(())
    }

    pub fn lock_seats(&self) -> Locked<GlobalName, Rc<WlSeatGlobal>> {
        self.seats.lock()
    }

    pub fn notify_all(&self, registry: &Rc<WlRegistry>) {
        let secure = registry.client.secure;
        let globals = self.registry.lock();
        macro_rules! emit {
            ($singleton:expr) => {
                for global in globals.values() {
                    if secure || !global.secure() {
                        if global.singleton() == $singleton {
                            registry.send_global(global);
                        }
                    }
                }
            };
        }
        emit!(true);
        emit!(false);
    }

    fn broadcast<F: Fn(&Rc<WlRegistry>)>(&self, state: &State, secure: bool, f: F) {
        state.clients.broadcast(secure, |c| {
            let registries = c.lock_registries();
            for registry in registries.values() {
                f(registry);
            }
            c.flush();
        });
    }

    fn take(&self, name: GlobalName, remove: bool) -> Result<Rc<dyn Global>, GlobalsError> {
        let res = if remove {
            self.registry.remove(&name)
        } else {
            self.registry.get(&name)
        };
        match res {
            Some(g) => Ok(g),
            None => Err(GlobalsError::GlobalDoesNotExist(name)),
        }
    }

    #[allow(dead_code)]
    pub fn get_output(&self, output: GlobalName) -> Result<Rc<WlOutputGlobal>, GlobalsError> {
        match self.outputs.get(&output) {
            Some(o) => Ok(o),
            _ => Err(GlobalsError::OutputDoesNotExist(output)),
        }
    }

    pub fn add_global<T: WaylandGlobal>(&self, state: &State, global: &Rc<T>) {
        global.clone().add(self);
        self.insert(state, global.clone())
    }

    pub fn add_global_no_broadcast<T: WaylandGlobal>(&self, global: &Rc<T>) {
        global.clone().add(self);
        self.insert_no_broadcast(global.clone());
    }
}

pub trait WaylandGlobal: Global + 'static {
    fn add(self: Rc<Self>, globals: &Globals) {
        let _ = globals;
    }
    fn remove(&self, globals: &Globals) {
        let _ = globals;
    }
}

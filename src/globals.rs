use crate::client::{Client, DynEventFormatter};
use crate::ifs::org_kde_kwin_server_decoration_manager::{
    OrgKdeKwinServerDecorationManagerError, OrgKdeKwinServerDecorationManagerGlobal,
};
use crate::ifs::wl_compositor::WlCompositorError;
use crate::ifs::wl_data_device_manager::WlDataDeviceManagerError;
use crate::ifs::wl_drm::{WlDrmError, WlDrmGlobal};
use crate::ifs::wl_output::{WlOutputError, WlOutputGlobal};
use crate::ifs::wl_registry::WlRegistry;
use crate::ifs::wl_seat::{WlSeatError, WlSeatGlobal};
use crate::ifs::wl_shm::WlShmError;
use crate::ifs::wl_subcompositor::WlSubcompositorError;
use crate::ifs::xdg_wm_base::XdgWmBaseError;
use crate::ifs::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Error;
use crate::ifs::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Error;
use crate::object::{Interface, ObjectId};
use crate::utils::copyhashmap::CopyHashMap;
use crate::{
    NumCell, State, WlCompositorGlobal, WlDataDeviceManagerGlobal, WlShmGlobal,
    WlSubcompositorGlobal, XdgWmBaseGlobal, ZwpLinuxDmabufV1Global, ZxdgDecorationManagerV1Global,
};
use ahash::AHashMap;
use std::cell::RefMut;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GlobalError {
    #[error("The requested global {0} does not exist")]
    GlobalDoesNotExist(GlobalName),
    #[error("An error occurred in a `wl_compositor` global")]
    WlCompositorError(#[source] Box<WlCompositorError>),
    #[error("An error occurred in a `wl_shm` global")]
    WlShmError(#[source] Box<WlShmError>),
    #[error("An error occurred in a `wl_subcompositor` global")]
    WlSubcompositorError(#[source] Box<WlSubcompositorError>),
    #[error("An error occurred in a `xdg_wm_base` global")]
    XdgWmBaseError(#[source] Box<XdgWmBaseError>),
    #[error("An error occurred in a `wl_output` global")]
    WlOutputError(#[source] Box<WlOutputError>),
    #[error("An error occurred in a `wl_seat` global")]
    WlSeatError(#[source] Box<WlSeatError>),
    #[error("The output with id {0} does not exist")]
    OutputDoesNotExist(GlobalName),
    #[error("An error occurred in a `wl_data_device_manager` global")]
    WlDataDeviceManagerError(#[source] Box<WlDataDeviceManagerError>),
    #[error("An error occurred in a `zwp_linux_dmabuf_v1` global")]
    ZwpLinuxDmabufV1Error(#[source] Box<ZwpLinuxDmabufV1Error>),
    #[error("An error occurred in a `wl_drm` global")]
    WlDrmError(#[source] Box<WlDrmError>),
    #[error("An error occurred in a `zxdg_decoration_manager_v1` global")]
    ZxdgDecorationManagerV1Error(#[source] Box<ZxdgDecorationManagerV1Error>),
    #[error("An error occurred in a `org_kde_kwin_server_decoration_manager` global")]
    OrgKdeKwinServerDecorationManagerError(#[source] Box<OrgKdeKwinServerDecorationManagerError>),
}

efrom!(GlobalError, WlCompositorError);
efrom!(GlobalError, WlShmError);
efrom!(GlobalError, WlSubcompositorError);
efrom!(GlobalError, XdgWmBaseError);
efrom!(GlobalError, WlOutputError);
efrom!(GlobalError, WlSeatError);
efrom!(GlobalError, ZwpLinuxDmabufV1Error);
efrom!(GlobalError, WlDrmError);
efrom!(GlobalError, WlDataDeviceManagerError);
efrom!(GlobalError, ZxdgDecorationManagerV1Error);
efrom!(GlobalError, OrgKdeKwinServerDecorationManagerError);

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

pub trait GlobalBind {
    fn bind<'a>(
        self: Rc<Self>,
        client: &'a Rc<Client>,
        id: ObjectId,
        version: u32,
    ) -> Result<(), GlobalError>;
}

pub trait Global: GlobalBind {
    fn name(&self) -> GlobalName;
    fn singleton(&self) -> bool;
    fn interface(&self) -> Interface;
    fn version(&self) -> u32;
    fn break_loops(&self) {}
}

pub struct Globals {
    next_name: NumCell<u32>,
    registry: CopyHashMap<GlobalName, Rc<dyn Global>>,
    outputs: CopyHashMap<GlobalName, Rc<WlOutputGlobal>>,
    seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
}

impl Globals {
    pub fn new() -> Self {
        let slf = Self {
            next_name: NumCell::new(1),
            registry: CopyHashMap::new(),
            outputs: Default::default(),
            seats: Default::default(),
        };
        macro_rules! add_singleton {
            ($name:ident) => {
                slf.add_global_no_broadcast(&Rc::new($name::new(slf.name())));
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
        slf
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
        self.broadcast(state, |r| r.global(&global));
    }

    pub fn get(&self, name: GlobalName) -> Result<Rc<dyn Global>, GlobalError> {
        self.take(name, false)
    }

    pub fn remove(&self, state: &State, name: GlobalName) -> Result<(), GlobalError> {
        let _global = self.take(name, true)?;
        self.broadcast(state, |r| r.global_remove(name));
        Ok(())
    }

    pub fn lock_seats(&self) -> RefMut<AHashMap<GlobalName, Rc<WlSeatGlobal>>> {
        self.seats.lock()
    }

    pub fn notify_all(&self, client: &Rc<Client>, registry: &Rc<WlRegistry>) {
        let globals = self.registry.lock();
        macro_rules! emit {
            ($singleton:expr) => {
                for global in globals.values() {
                    if global.singleton() == $singleton {
                        client.event(registry.global(global));
                    }
                }
            };
        }
        emit!(true);
        emit!(false);
    }

    fn broadcast<F: Fn(&Rc<WlRegistry>) -> DynEventFormatter>(&self, state: &State, f: F) {
        state.clients.broadcast(|c| {
            let registries = c.lock_registries();
            for registry in registries.values() {
                c.event(f(registry));
            }
            c.flush();
        });
    }

    fn take(&self, name: GlobalName, remove: bool) -> Result<Rc<dyn Global>, GlobalError> {
        let res = if remove {
            self.registry.remove(&name)
        } else {
            self.registry.get(&name)
        };
        match res {
            Some(g) => Ok(g),
            None => Err(GlobalError::GlobalDoesNotExist(name)),
        }
    }

    #[allow(dead_code)]
    pub fn get_output(&self, output: GlobalName) -> Result<Rc<WlOutputGlobal>, GlobalError> {
        match self.outputs.get(&output) {
            Some(o) => Ok(o),
            _ => Err(GlobalError::OutputDoesNotExist(output)),
        }
    }
}

pub trait AddGlobal<T> {
    fn add_global(&self, state: &State, global: &Rc<T>);

    fn add_global_no_broadcast(&self, global: &Rc<T>);

    fn remove_global(&self, state: &State, global: &T) -> Result<(), GlobalError>;
}

macro_rules! simple_add_global {
    ($ty:ty) => {
        impl AddGlobal<$ty> for Globals {
            fn add_global(&self, state: &State, global: &Rc<$ty>) {
                self.insert(state, global.clone())
            }

            fn add_global_no_broadcast(&self, global: &Rc<$ty>) {
                self.insert_no_broadcast(global.clone());
            }

            fn remove_global(&self, state: &State, global: &$ty) -> Result<(), GlobalError> {
                self.remove(state, global.name())
            }
        }
    };
}

simple_add_global!(WlCompositorGlobal);
simple_add_global!(WlShmGlobal);
simple_add_global!(WlSubcompositorGlobal);
simple_add_global!(XdgWmBaseGlobal);
simple_add_global!(WlDataDeviceManagerGlobal);
simple_add_global!(ZwpLinuxDmabufV1Global);
simple_add_global!(WlDrmGlobal);
simple_add_global!(ZxdgDecorationManagerV1Global);
simple_add_global!(OrgKdeKwinServerDecorationManagerGlobal);

macro_rules! dedicated_add_global {
    ($ty:ty, $field:ident) => {
        impl AddGlobal<$ty> for Globals {
            fn add_global(&self, state: &State, global: &Rc<$ty>) {
                self.insert(state, global.clone());
                self.$field.set(global.name(), global.clone());
            }

            fn add_global_no_broadcast(&self, global: &Rc<$ty>) {
                self.insert_no_broadcast(global.clone());
                self.$field.set(global.name(), global.clone());
            }

            fn remove_global(&self, state: &State, global: &$ty) -> Result<(), GlobalError> {
                self.$field.remove(&global.name());
                self.remove(state, global.name())
            }
        }
    };
}

dedicated_add_global!(WlOutputGlobal, outputs);
dedicated_add_global!(WlSeatGlobal, seats);

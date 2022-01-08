use crate::client::{Client, ClientError, DynEventFormatter, WlEvent};
use crate::ifs::wl_compositor::WlCompositorError;
use crate::ifs::wl_output::{WlOutputError, WlOutputGlobal};
use crate::ifs::wl_registry::WlRegistry;
use crate::ifs::wl_seat::{WlSeatError, WlSeatGlobal};
use crate::ifs::wl_shm::WlShmError;
use crate::ifs::wl_subcompositor::WlSubcompositorError;
use crate::ifs::xdg_wm_base::XdgWmBaseError;
use crate::object::{Interface, ObjectId};
use crate::utils::copyhashmap::CopyHashMap;
use crate::{NumCell, State, WlCompositorGlobal, WlDataDeviceManagerGlobal, WlShmGlobal, WlSubcompositorGlobal, XdgWmBaseGlobal};
use ahash::AHashSet;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::wl_data_device_manager::WlDataDeviceManagerError;

#[derive(Debug, Error)]
pub enum GlobalError {
    #[error("The requested global {0} does not exist")]
    GlobalDoesNotExist(GlobalName),
    #[error("An error occurred while trying to send all globals via a new registry")]
    SendAllError(#[source] Box<ClientError>),
    #[error("An error occurred in a wl_compositor")]
    WlCompositorError(#[source] Box<WlCompositorError>),
    #[error("An error occurred in a wl_shm")]
    WlShmError(#[source] Box<WlShmError>),
    #[error("An error occurred in a wl_subcompositor")]
    WlSubcompositorError(#[source] Box<WlSubcompositorError>),
    #[error("An error occurred in a xdg_wm_base")]
    XdgWmBaseError(#[source] Box<XdgWmBaseError>),
    #[error("An error occurred in a wl_output")]
    WlOutputError(#[source] Box<WlOutputError>),
    #[error("An error occurred in a wl_seat")]
    WlSeatError(#[source] Box<WlSeatError>),
    #[error("The output with id {0} does not exist")]
    OutputDoesNotExist(GlobalName),
    #[error("An error occurred in a wl_data_device_manager")]
    WlDataDeviceManagerError(#[source] Box<WlDataDeviceManagerError>),
}

efrom!(GlobalError, WlCompositorError, WlCompositorError);
efrom!(GlobalError, WlShmError, WlShmError);
efrom!(GlobalError, WlSubcompositorError, WlSubcompositorError);
efrom!(GlobalError, XdgWmBaseError, XdgWmBaseError);
efrom!(GlobalError, WlOutputError, WlOutputError);
efrom!(GlobalError, WlSeatError, WlSeatError);
efrom!(GlobalError, WlDataDeviceManagerError, WlDataDeviceManagerError);

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
    ) -> Pin<Box<dyn Future<Output = Result<(), GlobalError>> + 'a>>;
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
}

impl Globals {
    pub fn new() -> Self {
        Self {
            next_name: NumCell::new(1),
            registry: CopyHashMap::new(),
            outputs: Default::default(),
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

    async fn insert<'a>(&'a self, state: &'a State, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
        self.broadcast(state, |r| r.global(&global)).await;
    }

    pub fn get(&self, name: GlobalName) -> Result<Rc<dyn Global>, GlobalError> {
        self.take(name, false)
    }

    pub async fn remove(&self, state: &State, name: GlobalName) -> Result<(), GlobalError> {
        let _global = self.take(name, true)?;
        self.broadcast(state, |r| r.global_remove(name)).await;
        Ok(())
    }

    pub async fn notify_all(
        &self,
        client: &Client,
        registry: &Rc<WlRegistry>,
    ) -> Result<(), GlobalError> {
        let globals = self.registry.lock();
        macro_rules! emit {
            ($singleton:expr) => {
                for global in globals.values() {
                    if global.singleton() == $singleton {
                        if let Err(e) = client.event(registry.global(global)).await {
                            return Err(GlobalError::SendAllError(Box::new(e)));
                        }
                    }
                }
            };
        }
        emit!(true);
        emit!(false);
        Ok(())
    }

    async fn broadcast<F: Fn(&Rc<WlRegistry>) -> DynEventFormatter>(&self, state: &State, f: F) {
        let mut clients_to_check = AHashSet::new();
        state.clients.broadcast(|c| {
            let registries = c.lock_registries();
            for registry in registries.values() {
                if c.event_locked(f(registry)) {
                    clients_to_check.insert(c.id);
                }
            }
            if c.event2_locked(WlEvent::Flush) {
                clients_to_check.insert(c.id);
            }
        });
        for client in clients_to_check.drain() {
            if let Ok(c) = state.clients.get(client) {
                let _ = c.check_queue_size().await;
            }
        }
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
    type RemoveGlobal<'a>: Future<Output = Result<(), GlobalError>> + 'a;
    type AddGlobal<'a>: Future<Output = ()> + 'a;

    fn add_global<'a>(&'a self, state: &'a State, global: &'a Rc<T>) -> Self::AddGlobal<'a>;

    fn add_global_no_broadcast(&self, global: &Rc<T>);

    fn remove_global<'a>(&'a self, state: &'a State, global: &'a T) -> Self::RemoveGlobal<'a>;
}

macro_rules! simple_add_global {
    ($ty:ty) => {
        impl AddGlobal<$ty> for Globals {
            type RemoveGlobal<'a> = impl Future<Output = Result<(), GlobalError>> + 'a;
            type AddGlobal<'a> = impl Future<Output = ()> + 'a;

            fn add_global<'a>(
                &'a self,
                state: &'a State,
                global: &'a Rc<$ty>,
            ) -> Self::AddGlobal<'a> {
                self.insert(state, global.clone())
            }

            fn add_global_no_broadcast(&self, global: &Rc<$ty>) {
                self.insert_no_broadcast(global.clone());
            }

            fn remove_global<'a>(
                &'a self,
                state: &'a State,
                global: &'a $ty,
            ) -> Self::RemoveGlobal<'a> {
                self.remove(state, global.name())
            }
        }
    };
}

simple_add_global!(WlSeatGlobal);
simple_add_global!(WlCompositorGlobal);
simple_add_global!(WlShmGlobal);
simple_add_global!(WlSubcompositorGlobal);
simple_add_global!(XdgWmBaseGlobal);
simple_add_global!(WlDataDeviceManagerGlobal);

macro_rules! dedicated_add_global {
    ($ty:ty, $field:ident) => {
        impl AddGlobal<$ty> for Globals {
            type RemoveGlobal<'a> = impl Future<Output = Result<(), GlobalError>> + 'a;
            type AddGlobal<'a> = impl Future<Output = ()> + 'a;

            fn add_global<'a>(
                &'a self,
                state: &'a State,
                global: &'a Rc<$ty>,
            ) -> Self::AddGlobal<'a> {
                async move {
                    self.insert(state, global.clone()).await;
                    self.$field.set(global.name(), global.clone());
                }
            }

            fn add_global_no_broadcast(&self, global: &Rc<$ty>) {
                self.insert_no_broadcast(global.clone());
                self.$field.set(global.name(), global.clone());
            }

            fn remove_global<'a>(
                &'a self,
                state: &'a State,
                global: &'a $ty,
            ) -> Self::RemoveGlobal<'a> {
                self.$field.remove(&global.name());
                self.remove(state, global.name())
            }
        }
    };
}

dedicated_add_global!(WlOutputGlobal, outputs);

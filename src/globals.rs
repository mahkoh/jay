use crate::client::{Client, ClientError, DynEventFormatter};
use crate::ifs::wl_compositor::WlCompositorError;
use crate::ifs::wl_registry::WlRegistry;
use crate::ifs::wl_shm::WlShmError;
use crate::ifs::wl_subcompositor::WlSubcompositorError;
use crate::ifs::xdg_wm_base::XdgWmBaseError;
use crate::object::{Interface, ObjectId};
use crate::utils::copyhashmap::CopyHashMap;
use crate::{NumCell, State};
use ahash::AHashSet;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use thiserror::Error;

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
}

efrom!(GlobalError, WlCompositorError, WlCompositorError);
efrom!(GlobalError, WlShmError, WlShmError);
efrom!(GlobalError, WlSubcompositorError, WlSubcompositorError);
efrom!(GlobalError, XdgWmBaseError, XdgWmBaseError);

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
    fn interface(&self) -> Interface;
    fn version(&self) -> u32;
    fn pre_remove(&self);
}

pub struct Globals {
    next_name: NumCell<u32>,
    registry: CopyHashMap<GlobalName, Rc<dyn Global>>,
}

impl Globals {
    pub fn new() -> Self {
        Self {
            next_name: NumCell::new(1),
            registry: CopyHashMap::new(),
        }
    }

    pub fn name(&self) -> GlobalName {
        let id = self.next_name.fetch_add(1);
        if id == 0 {
            panic!("Global names overflowed");
        }
        GlobalName(id)
    }

    pub fn insert_no_broadcast<'a>(&'a self, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
    }

    fn insert_no_broadcast_<'a>(&'a self, global: &Rc<dyn Global>) {
        self.registry.set(global.name(), global.clone());
    }

    pub async fn insert<'a>(&'a self, state: &'a State, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
        self.broadcast(state, |r| r.global(&global)).await;
    }

    pub fn get(&self, name: GlobalName) -> Result<Rc<dyn Global>, GlobalError> {
        self.take(name, false)
    }

    pub async fn remove(
        &self,
        state: &State,
        name: GlobalName,
    ) -> Result<Rc<dyn Global>, GlobalError> {
        let global = self.take(name, true)?;
        global.pre_remove();
        self.broadcast(state, |r| r.global_remove(name)).await;
        Ok(global)
    }

    pub async fn notify_all(
        &self,
        client: &Client,
        registry: &Rc<WlRegistry>,
    ) -> Result<(), GlobalError> {
        let globals = self.registry.lock();
        for global in globals.values() {
            if let Err(e) = client.event(registry.global(global)).await {
                return Err(GlobalError::SendAllError(Box::new(e)));
            }
        }
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
}

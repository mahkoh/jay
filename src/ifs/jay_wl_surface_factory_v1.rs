use crate::client::Client;
use crate::ifs::jay_wl_surface_factory_v1::WlSurfaceFactoryState::Abandoned;
use crate::ifs::jay_wl_surface_factory_v1::WlSurfaceFactoryState::Assigned;
use crate::ifs::jay_wl_surface_factory_v1::WlSurfaceFactoryState::Initial;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayWlSurfaceFactoryV1Id;
use crate::wire::jay_wl_surface_factory_v1::*;
use jay_proc::Object;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

#[derive(Object)]
pub struct JayWlSurfaceFactoryV1 {
    id: JayWlSurfaceFactoryV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
    fs: Cell<WlSurfaceFactoryState>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
enum WlSurfaceFactoryState {
    #[default]
    Initial,
    Assigned,
    Abandoned,
}

impl JayWlSurfaceFactoryV1 {
    pub fn new(id: JayWlSurfaceFactoryV1Id, version: Version, client: &Rc<Client>) -> Rc<Self> {
        let obj = Rc::new(Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            fs: Default::default(),
        });
        track!(client, obj);
        obj
    }

    pub fn assign(&self) -> Result<(), JayWlSurfaceFactoryV1Error> {
        match self.fs.get() {
            Initial => self.fs.set(Assigned),
            Assigned => return Err(JayWlSurfaceFactoryV1Error::AlreadyAssigned),
            Abandoned => return Err(JayWlSurfaceFactoryV1Error::AlreadyAbandoned),
        }
        Ok(())
    }

    pub fn abandon(&self) {
        if self.fs.get() != Assigned {
            log::warn!("Factory is not assigned");
        }
        self.fs.set(Abandoned);
    }

    pub fn send_surface(&self) -> Rc<WlSurface> {
        if self.fs.get() != Assigned {
            log::warn!("Factory is not assigned");
        }
        let surface = Rc::new_cyclic(|slf| {
            WlSurface::new(self.client.new_id(self), &self.client, self.version, slf)
        });
        track!(self.client, surface);
        self.client.add_server_obj(&surface);
        self.client.event(Surface {
            self_id: self.id,
            id: surface.id,
        });
        surface
    }
}

impl JayWlSurfaceFactoryV1RequestHandler for JayWlSurfaceFactoryV1 {
    type Error = JayWlSurfaceFactoryV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.fs.get() == Assigned {
            return Err(JayWlSurfaceFactoryV1Error::StillAssigned);
        }
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JayWlSurfaceFactoryV1Error {
    #[error("The wl_surface factory is already assigned to a producer")]
    AlreadyAssigned,
    #[error("The wl_surface factory is already abandoned")]
    AlreadyAbandoned,
    #[error("The wl_surface factory is still assigned to a producer")]
    StillAssigned,
}

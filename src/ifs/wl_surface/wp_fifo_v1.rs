use crate::client::Client;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpFifoV1Id;
use crate::wire::wp_fifo_v1::*;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Object)]
pub struct WpFifoV1 {
    id: WpFifoV1Id,
    client: Rc<Client>,
    surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl WpFifoV1 {
    pub fn new(id: WpFifoV1Id, version: Version, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpFifoV1Error> {
        if self.surface.fifo.is_some() {
            return Err(WpFifoV1Error::Exists);
        }
        self.surface.fifo.set(Some(self.clone()));
        Ok(())
    }
}

impl WpFifoV1RequestHandler for WpFifoV1 {
    type Error = WpFifoV1Error;

    fn set_barrier(&self, _req: SetBarrier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().fifo_barrier_set = true;
        Ok(())
    }

    fn wait_barrier(&self, _req: WaitBarrier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().fifo_barrier_wait = true;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.fifo.take();
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WpFifoV1Error {
    #[error("The surface already has a fifo extension attached")]
    Exists,
}

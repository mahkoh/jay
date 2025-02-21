use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{WpFifoV1Id, wp_fifo_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFifoV1 {
    pub id: WpFifoV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
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
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpFifoV1;
    version = self.version;
}

impl Object for WpFifoV1 {}

simple_add_obj!(WpFifoV1);

#[derive(Debug, Error)]
pub enum WpFifoV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a fifo extension attached")]
    Exists,
}
efrom!(WpFifoV1Error, ClientError);

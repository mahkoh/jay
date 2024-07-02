pub use crate::wire::{wp_presentation::*, WpPresentationId};
use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_presentation_feedback::WpPresentationFeedback,
        leaks::Tracker,
        object::{Object, Version},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::c,
};

pub struct WpPresentationGlobal {
    pub name: GlobalName,
}

impl WpPresentationGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpPresentationId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpPresentationError> {
        let obj = Rc::new(WpPresentation {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.send_clock_id();
        Ok(())
    }
}

global_base!(WpPresentationGlobal, WpPresentation, WpPresentationError);

impl Global for WpPresentationGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpPresentationGlobal);

pub struct WpPresentation {
    pub id: WpPresentationId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpPresentation {
    fn send_clock_id(&self) {
        self.client.event(ClockId {
            self_id: self.id,
            clk_id: c::CLOCK_MONOTONIC as _,
        });
    }
}

impl WpPresentationRequestHandler for WpPresentation {
    type Error = WpPresentationError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn feedback(&self, req: Feedback, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let fb = Rc::new(WpPresentationFeedback {
            id: req.callback,
            client: self.client.clone(),
            _surface: surface.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, fb);
        self.client.add_client_obj(&fb)?;
        surface.add_presentation_feedback(&fb);
        Ok(())
    }
}

object_base! {
    self = WpPresentation;
    version = self.version;
}

impl Object for WpPresentation {}

simple_add_obj!(WpPresentation);

#[derive(Debug, Error)]
pub enum WpPresentationError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpPresentationError, ClientError);

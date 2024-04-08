pub use crate::wire::{wp_presentation::*, WpPresentationId};
use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_presentation_feedback::WpPresentationFeedback,
        leaks::Tracker,
        object::{Object, Version},
        utils::buffd::{MsgParser, MsgParserError},
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
        _version: Version,
    ) -> Result<(), WpPresentationError> {
        let obj = Rc::new(WpPresentation {
            id,
            client: client.clone(),
            tracker: Default::default(),
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
}

impl WpPresentation {
    fn send_clock_id(&self) {
        self.client.event(ClockId {
            self_id: self.id,
            clk_id: c::CLOCK_MONOTONIC as _,
        });
    }

    pub fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpPresentationError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    pub fn feedback(&self, parser: MsgParser<'_, '_>) -> Result<(), WpPresentationError> {
        let req: Feedback = self.client.parse(self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let fb = Rc::new(WpPresentationFeedback {
            id: req.callback,
            client: self.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
        });
        track!(self.client, fb);
        self.client.add_client_obj(&fb)?;
        surface.add_presentation_feedback(&fb);
        Ok(())
    }
}

object_base! {
    self = WpPresentation;

    DESTROY => destroy,
    FEEDBACK => feedback,
}

impl Object for WpPresentation {}

simple_add_obj!(WpPresentation);

#[derive(Debug, Error)]
pub enum WpPresentationError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpPresentationError, MsgParserError);
efrom!(WpPresentationError, ClientError);

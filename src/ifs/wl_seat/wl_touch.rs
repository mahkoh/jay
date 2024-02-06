use {
    crate::{
        client::ClientError,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_touch::*, WlTouchId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[allow(dead_code)]
const DOWN: u32 = 0;
#[allow(dead_code)]
const UP: u32 = 1;
#[allow(dead_code)]
const MOTION: u32 = 2;
#[allow(dead_code)]
const FRAME: u32 = 3;
#[allow(dead_code)]
const CANCEL: u32 = 4;
#[allow(dead_code)]
const SHAPE: u32 = 5;
#[allow(dead_code)]
const ORIENTATION: u32 = 6;

pub struct WlTouch {
    id: WlTouchId,
    seat: Rc<WlSeat>,
    pub tracker: Tracker<Self>,
}

impl WlTouch {
    pub fn new(id: WlTouchId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
            tracker: Default::default(),
        }
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), WlTouchError> {
        let _req: Release = self.seat.client.parse(self, parser)?;
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WlTouch;

    RELEASE => release if self.seat.version >= 3,
}

impl Object for WlTouch {}

simple_add_obj!(WlTouch);

#[derive(Debug, Error)]
pub enum WlTouchError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(WlTouchError, ClientError);
efrom!(WlTouchError, MsgParserError);

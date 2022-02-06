use crate::client::ClientError;
use crate::cursor::Cursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::WlSeat;
use crate::ifs::wl_surface::WlSurfaceError;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_pointer::*;
use crate::wire::{WlPointerId, WlSurfaceId};
use std::rc::Rc;
use thiserror::Error;

#[allow(dead_code)]
const ROLE: u32 = 0;

pub(super) const RELEASED: u32 = 0;
pub(super) const PRESSED: u32 = 1;

pub(super) const VERTICAL_SCROLL: u32 = 0;
pub(super) const HORIZONTAL_SCROLL: u32 = 1;

#[allow(dead_code)]
const WHEEL: u32 = 0;
#[allow(dead_code)]
const FINGER: u32 = 1;
#[allow(dead_code)]
const CONTINUOUS: u32 = 2;
#[allow(dead_code)]
const WHEEL_TILT: u32 = 3;

pub const POINTER_FRAME_SINCE_VERSION: u32 = 5;

pub struct WlPointer {
    id: WlPointerId,
    seat: Rc<WlSeat>,
}

impl WlPointer {
    pub fn new(id: WlPointerId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
        }
    }

    pub fn send_enter(&self, serial: u32, surface: WlSurfaceId, x: Fixed, y: Fixed) {
        self.seat.client.event(Enter {
            self_id: self.id,
            serial,
            surface,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn send_leave(&self, serial: u32, surface: WlSurfaceId) {
        self.seat.client.event(Leave {
            self_id: self.id,
            serial,
            surface,
        })
    }

    pub fn send_motion(&self, time: u32, x: Fixed, y: Fixed) {
        self.seat.client.event(Motion {
            self_id: self.id,
            time,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn send_button(&self, serial: u32, time: u32, button: u32, state: u32) {
        self.seat.client.event(Button {
            self_id: self.id,
            serial,
            time,
            button,
            state,
        })
    }

    pub fn send_axis(&self, time: u32, axis: u32, value: Fixed) {
        self.seat.client.event(Axis {
            self_id: self.id,
            time,
            axis,
            value,
        })
    }

    #[allow(dead_code)]
    pub fn send_frame(&self) {
        self.seat.client.event(Frame { self_id: self.id })
    }

    #[allow(dead_code)]
    pub fn send_axis_source(&self, axis_source: u32) {
        self.seat.client.event(AxisSource {
            self_id: self.id,
            axis_source,
        })
    }

    #[allow(dead_code)]
    pub fn send_axis_stop(&self, time: u32, axis: u32) {
        self.seat.client.event(AxisStop {
            self_id: self.id,
            time,
            axis,
        })
    }

    #[allow(dead_code)]
    pub fn send_axis_discrete(&self, axis: u32, discrete: i32) {
        self.seat.client.event(AxisDiscrete {
            self_id: self.id,
            axis,
            discrete,
        })
    }

    fn set_cursor(&self, parser: MsgParser<'_, '_>) -> Result<(), SetCursorError> {
        let req: SetCursor = self.seat.client.parse(self, parser)?;
        let mut cursor_opt = None;
        if req.surface.is_some() {
            let surface = self.seat.client.lookup(req.surface)?;
            let cursor = surface.get_cursor(&self.seat.global)?;
            cursor.set_hotspot(req.hotspot_x, req.hotspot_y);
            cursor_opt = Some(cursor as Rc<dyn Cursor>);
        }
        let pointer_node = match self.seat.global.pointer_stack.borrow().last().cloned() {
            Some(n) => n,
            _ => {
                // cannot happen
                return Ok(());
            }
        };
        if pointer_node.client_id() != Some(self.seat.client.id) {
            return Ok(());
        }
        self.seat.global.set_cursor(cursor_opt);
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.seat.client.parse(self, parser)?;
        self.seat.pointers.remove(&self.id);
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    WlPointer, WlPointerError;

    SET_CURSOR => set_cursor,
    RELEASE => release,
}

impl Object for WlPointer {
    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}

simple_add_obj!(WlPointer);

#[derive(Debug, Error)]
pub enum WlPointerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `set_cursor` request")]
    SetCursorError(#[from] SetCursorError),
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
}
efrom!(WlPointerError, ClientError);

#[derive(Debug, Error)]
pub enum SetCursorError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(SetCursorError, ParseError, MsgParserError);
efrom!(SetCursorError, ClientError);
efrom!(SetCursorError, WlSurfaceError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseError, MsgParserError);
efrom!(ReleaseError, ClientError, ClientError);

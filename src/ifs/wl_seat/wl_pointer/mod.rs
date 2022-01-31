mod types;

use crate::client::DynEventFormatter;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::WlSeatObj;
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const SET_CURSOR: u32 = 0;
const RELEASE: u32 = 1;

const ENTER: u32 = 0;
const LEAVE: u32 = 1;
const MOTION: u32 = 2;
const BUTTON: u32 = 3;
const AXIS: u32 = 4;
const FRAME: u32 = 5;
const AXIS_SOURCE: u32 = 6;
const AXIS_STOP: u32 = 7;
const AXIS_DISCRETE: u32 = 8;

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

id!(WlPointerId);

pub struct WlPointer {
    id: WlPointerId,
    seat: Rc<WlSeatObj>,
}

impl WlPointer {
    pub fn new(id: WlPointerId, seat: &Rc<WlSeatObj>) -> Self {
        Self {
            id,
            seat: seat.clone(),
        }
    }

    pub fn enter(
        self: &Rc<Self>,
        serial: u32,
        surface: WlSurfaceId,
        x: Fixed,
        y: Fixed,
    ) -> DynEventFormatter {
        Box::new(Enter {
            obj: self.clone(),
            serial,
            surface,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn leave(self: &Rc<Self>, serial: u32, surface: WlSurfaceId) -> DynEventFormatter {
        Box::new(Leave {
            obj: self.clone(),
            serial,
            surface,
        })
    }

    pub fn motion(self: &Rc<Self>, time: u32, x: Fixed, y: Fixed) -> DynEventFormatter {
        Box::new(Motion {
            obj: self.clone(),
            time,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn button(
        self: &Rc<Self>,
        serial: u32,
        time: u32,
        button: u32,
        state: u32,
    ) -> DynEventFormatter {
        Box::new(Button {
            obj: self.clone(),
            serial,
            time,
            button,
            state,
        })
    }

    pub fn axis(self: &Rc<Self>, time: u32, axis: u32, value: Fixed) -> DynEventFormatter {
        Box::new(Axis {
            obj: self.clone(),
            time,
            axis,
            value,
        })
    }

    #[allow(dead_code)]
    pub fn frame(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Frame { obj: self.clone() })
    }

    #[allow(dead_code)]
    pub fn axis_source(self: &Rc<Self>, axis_source: u32) -> DynEventFormatter {
        Box::new(AxisSource {
            obj: self.clone(),
            axis_source,
        })
    }

    #[allow(dead_code)]
    pub fn axis_stop(self: &Rc<Self>, time: u32, axis: u32) -> DynEventFormatter {
        Box::new(AxisStop {
            obj: self.clone(),
            time,
            axis,
        })
    }

    #[allow(dead_code)]
    pub fn axis_discrete(self: &Rc<Self>, axis: u32, discrete: i32) -> DynEventFormatter {
        Box::new(AxisDiscrete {
            obj: self.clone(),
            axis,
            discrete,
        })
    }

    fn set_cursor(&self, parser: MsgParser<'_, '_>) -> Result<(), SetCursorError> {
        let req: SetCursor = self.seat.client.parse(self, parser)?;
        let mut cursor_opt = None;
        if req.surface.is_some() {
            let surface = self.seat.client.get_surface(req.surface)?;
            let cursor = surface.get_cursor(&self.seat.global)?;
            cursor.set_hotspot(req.hotspot_x, req.hotspot_y);
            cursor_opt = Some(cursor);
        }
        let pointer_node = match self.seat.global.pointer_stack.borrow().last().cloned() {
            Some(n) => n,
            _ => {
                // cannot happen
                return Ok(());
            },
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

    fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlPointerError> {
        match request {
            SET_CURSOR => self.set_cursor(parser)?,
            RELEASE => self.release(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlPointer);

impl Object for WlPointer {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlPointer
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}

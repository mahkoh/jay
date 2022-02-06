mod types;

use crate::client::DynEventFormatter;
use crate::cursor::Cursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::WlSeat;
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;
use crate::wire::wl_pointer::*;

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

    pub fn enter(
        self: &Rc<Self>,
        serial: u32,
        surface: WlSurfaceId,
        x: Fixed,
        y: Fixed,
    ) -> DynEventFormatter {
        Box::new(Enter {
            self_id: self.id,
            serial,
            surface,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn leave(self: &Rc<Self>, serial: u32, surface: WlSurfaceId) -> DynEventFormatter {
        Box::new(Leave {
            self_id: self.id,
            serial,
            surface,
        })
    }

    pub fn motion(self: &Rc<Self>, time: u32, x: Fixed, y: Fixed) -> DynEventFormatter {
        Box::new(Motion {
            self_id: self.id,
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
            self_id: self.id,
            serial,
            time,
            button,
            state,
        })
    }

    pub fn axis(self: &Rc<Self>, time: u32, axis: u32, value: Fixed) -> DynEventFormatter {
        Box::new(Axis {
            self_id: self.id,
            time,
            axis,
            value,
        })
    }

    #[allow(dead_code)]
    pub fn frame(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Frame { self_id: self.id })
    }

    #[allow(dead_code)]
    pub fn axis_source(self: &Rc<Self>, axis_source: u32) -> DynEventFormatter {
        Box::new(AxisSource {
            self_id: self.id,
            axis_source,
        })
    }

    #[allow(dead_code)]
    pub fn axis_stop(self: &Rc<Self>, time: u32, axis: u32) -> DynEventFormatter {
        Box::new(AxisStop {
            self_id: self.id,
            time,
            axis,
        })
    }

    #[allow(dead_code)]
    pub fn axis_discrete(self: &Rc<Self>, axis: u32, discrete: i32) -> DynEventFormatter {
        Box::new(AxisDiscrete {
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

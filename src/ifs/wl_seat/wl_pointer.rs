use {
    crate::{
        client::ClientError,
        cursor::Cursor,
        fixed::Fixed,
        ifs::{wl_seat::WlSeat, wl_surface::WlSurfaceError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlPointerId, WlSurfaceId, wl_pointer::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

#[expect(dead_code)]
const ROLE: u32 = 0;

pub(super) const RELEASED: u32 = 0;
pub const PRESSED: u32 = 1;

pub const VERTICAL_SCROLL: usize = 0;
pub const HORIZONTAL_SCROLL: usize = 1;

pub const WHEEL: u32 = 0;
pub const FINGER: u32 = 1;
pub const CONTINUOUS: u32 = 2;
pub const WHEEL_TILT: u32 = 3;

pub const IDENTICAL: u32 = 0;
pub const INVERTED: u32 = 1;

pub const POINTER_FRAME_SINCE_VERSION: Version = Version(5);
pub const AXIS_SOURCE_SINCE_VERSION: Version = Version(5);
pub const AXIS_DISCRETE_SINCE_VERSION: Version = Version(5);
pub const AXIS_STOP_SINCE_VERSION: Version = Version(5);
pub const WHEEL_TILT_SINCE_VERSION: Version = Version(6);
pub const AXIS_VALUE120_SINCE_VERSION: Version = Version(8);
pub const AXIS_RELATIVE_DIRECTION_SINCE_VERSION: Version = Version(9);

#[derive(Default, Debug)]
pub struct PendingScroll {
    pub v120: [Cell<Option<i32>>; 2],
    pub inverted: [Cell<bool>; 2],
    pub px: [Cell<Option<Fixed>>; 2],
    pub stop: [Cell<bool>; 2],
    pub source: Cell<Option<u32>>,
    pub time_usec: Cell<u64>,
}

impl PendingScroll {
    pub fn take(&self) -> Self {
        Self {
            v120: [
                Cell::new(self.v120[0].take()),
                Cell::new(self.v120[1].take()),
            ],
            inverted: [
                Cell::new(self.inverted[0].take()),
                Cell::new(self.inverted[1].take()),
            ],
            px: [Cell::new(self.px[0].take()), Cell::new(self.px[1].take())],
            stop: [
                Cell::new(self.stop[0].take()),
                Cell::new(self.stop[1].take()),
            ],
            source: Cell::new(self.source.take()),
            time_usec: Cell::new(self.time_usec.take()),
        }
    }
}

pub struct WlPointer {
    id: WlPointerId,
    pub seat: Rc<WlSeat>,
    pub tracker: Tracker<Self>,
    last_motion: Cell<(Fixed, Fixed)>,
    pub v120_accumulator: [Cell<i32>; 2],
}

impl WlPointer {
    pub fn new(id: WlPointerId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
            tracker: Default::default(),
            last_motion: Default::default(),
            v120_accumulator: Default::default(),
        }
    }

    pub fn send_enter(&self, serial: u64, surface: WlSurfaceId, mut x: Fixed, mut y: Fixed) {
        self.last_motion.set((x, y));
        for accumulator in &self.v120_accumulator {
            accumulator.set(0);
        }
        logical_to_client_wire_scale!(self.seat.client, x, y);
        self.seat.client.event(Enter {
            self_id: self.id,
            serial: serial as u32,
            surface,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn send_leave(&self, serial: u64, surface: WlSurfaceId) {
        self.seat.client.event(Leave {
            self_id: self.id,
            serial: serial as u32,
            surface,
        })
    }

    pub fn send_motion(&self, time: u32, mut x: Fixed, mut y: Fixed) {
        if self.last_motion.replace((x, y)) == (x, y) {
            return;
        }
        logical_to_client_wire_scale!(self.seat.client, x, y);
        self.seat.client.event(Motion {
            self_id: self.id,
            time,
            surface_x: x,
            surface_y: y,
        })
    }

    pub fn send_button(&self, serial: u64, time: u32, button: u32, state: u32) {
        self.seat.client.event(Button {
            self_id: self.id,
            serial: serial as u32,
            time,
            button,
            state,
        })
    }

    pub fn send_axis_relative_direction(&self, axis: u32, direction: u32) {
        self.seat.client.event(AxisRelativeDirection {
            self_id: self.id,
            axis,
            direction,
        })
    }

    pub fn send_axis(&self, time: u32, axis: u32, mut value: Fixed) {
        logical_to_client_wire_scale!(self.seat.client, value);
        self.seat.client.event(Axis {
            self_id: self.id,
            time,
            axis,
            value,
        })
    }

    pub fn send_frame(&self) {
        self.seat.client.event(Frame { self_id: self.id })
    }

    pub fn send_axis_source(&self, axis_source: u32) {
        self.seat.client.event(AxisSource {
            self_id: self.id,
            axis_source,
        })
    }

    pub fn send_axis_stop(&self, time: u32, axis: u32) {
        self.seat.client.event(AxisStop {
            self_id: self.id,
            time,
            axis,
        })
    }

    pub fn send_axis_discrete(&self, axis: u32, discrete: i32) {
        self.seat.client.event(AxisDiscrete {
            self_id: self.id,
            axis,
            discrete,
        })
    }

    pub fn send_axis_value120(&self, axis: u32, value120: i32) {
        self.seat.client.event(AxisValue120 {
            self_id: self.id,
            axis,
            value120,
        })
    }
}

impl WlPointerRequestHandler for WlPointer {
    type Error = WlPointerError;

    fn set_cursor(&self, mut req: SetCursor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.seat.client.map_serial(req.serial).is_none() {
            log::warn!("Client tried to set_cursor with an invalid serial");
            return Ok(());
        }
        let mut cursor_opt = None;
        if req.surface.is_some() {
            client_wire_scale_to_logical!(self.seat.client, req.hotspot_x, req.hotspot_y);
            let surface = self.seat.client.lookup(req.surface)?;
            let cursor = surface.get_cursor(&self.seat.global.pointer_cursor)?;
            cursor.set_hotspot(req.hotspot_x, req.hotspot_y);
            cursor_opt = Some(cursor as Rc<dyn Cursor>);
        }
        let pointer_node = match self.seat.global.pointer_node() {
            Some(n) => n,
            _ => {
                // cannot happen
                log::warn!("ignoring wl_pointer.set_cursor (1)");
                return Ok(());
            }
        };
        if pointer_node.node_client_id() != Some(self.seat.client.id) {
            // log::warn!("ignoring wl_pointer.set_cursor (2)");
            return Ok(());
        }
        // https://gitlab.freedesktop.org/wayland/wayland/-/issues/439
        // if req.serial != self.seat.client.last_enter_serial.get() {
        //     log::warn!(
        //         "ignoring wl_pointer.set_cursor (3) ({} != {})",
        //         req.serial,
        //         self.seat.client.last_enter_serial.get(),
        //     );
        //     return Ok(());
        // }
        self.seat.global.pointer_cursor().set(cursor_opt);
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.seat.pointers.remove(&self.id);
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WlPointer;
    version = self.seat.version;
}

impl Object for WlPointer {}

dedicated_add_obj!(WlPointer, WlPointerId, pointers);

#[derive(Debug, Error)]
pub enum WlPointerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(WlPointerError, ClientError);
efrom!(WlPointerError, WlSurfaceError);

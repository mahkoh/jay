use {
    crate::{
        client::{Client, ClientError},
        cursor::Cursor,
        fixed::Fixed,
        ifs::{
            wl_seat::tablet::{
                zwp_tablet_seat_v2::ZwpTabletSeatV2, zwp_tablet_v2::ZwpTabletV2,
                TabletToolCapability, TabletToolOpt, TabletToolType, ToolButtonState,
            },
            wl_surface::{WlSurface, WlSurfaceError},
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_tablet_tool_v2::*, ZwpTabletToolV2Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpTabletToolV2 {
    pub id: ZwpTabletToolV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<ZwpTabletSeatV2>,
    pub tool: Rc<TabletToolOpt>,
    pub entered: Cell<bool>,
}

pub const BTN_TOOL_PEN: u32 = 0x140;
pub const BTN_TOOL_RUBBER: u32 = 0x141;
pub const BTN_TOOL_BRUSH: u32 = 0x142;
pub const BTN_TOOL_PENCIL: u32 = 0x143;
pub const BTN_TOOL_AIRBRUSH: u32 = 0x144;
pub const BTN_TOOL_FINGER: u32 = 0x145;
pub const BTN_TOOL_MOUSE: u32 = 0x146;
pub const BTN_TOOL_LENS: u32 = 0x147;

impl ZwpTabletToolV2 {
    pub fn detach(&self) {
        if let Some(tool) = self.tool.get() {
            tool.bindings.remove(&self.seat);
        }
    }

    pub fn send_type(&self, tool_type: TabletToolType) {
        self.client.event(Type {
            self_id: self.id,
            tool_type: match tool_type {
                TabletToolType::Pen => BTN_TOOL_PEN,
                TabletToolType::Eraser => BTN_TOOL_RUBBER,
                TabletToolType::Brush => BTN_TOOL_BRUSH,
                TabletToolType::Pencil => BTN_TOOL_PENCIL,
                TabletToolType::Airbrush => BTN_TOOL_AIRBRUSH,
                TabletToolType::Finger => BTN_TOOL_FINGER,
                TabletToolType::Mouse => BTN_TOOL_MOUSE,
                TabletToolType::Lens => BTN_TOOL_LENS,
            },
        });
    }

    pub fn send_hardware_serial(&self, serial: u64) {
        self.client.event(HardwareSerial {
            self_id: self.id,
            hardware_serial_hi: (serial >> 32) as _,
            hardware_serial_lo: serial as _,
        });
    }

    pub fn send_hardware_id_wacom(&self, id: u64) {
        self.client.event(HardwareIdWacom {
            self_id: self.id,
            hardware_id_hi: (id >> 32) as _,
            hardware_id_lo: id as _,
        });
    }

    pub fn send_capability(&self, capability: TabletToolCapability) {
        self.client.event(Capability {
            self_id: self.id,
            capability: match capability {
                TabletToolCapability::Tilt => 1,
                TabletToolCapability::Pressure => 2,
                TabletToolCapability::Distance => 3,
                TabletToolCapability::Rotation => 4,
                TabletToolCapability::Slider => 5,
                TabletToolCapability::Wheel => 6,
            },
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_removed(&self) {
        self.client.event(Removed { self_id: self.id });
    }

    pub fn send_proximity_in(&self, serial: u32, tablet: &ZwpTabletV2, surface: &WlSurface) {
        self.entered.set(true);
        self.client.event(ProximityIn {
            self_id: self.id,
            serial,
            tablet: tablet.id,
            surface: surface.id,
        });
    }

    pub fn send_proximity_out(&self) {
        self.entered.set(false);
        self.client.event(ProximityOut { self_id: self.id });
    }

    pub fn send_down(&self, serial: u32) {
        self.client.event(Down {
            self_id: self.id,
            serial,
        });
    }

    pub fn send_up(&self) {
        self.client.event(Up { self_id: self.id });
    }

    pub fn send_motion(&self, x: Fixed, y: Fixed) {
        self.client.event(Motion {
            self_id: self.id,
            x,
            y,
        });
    }

    pub fn send_pressure(&self, pressure: u32) {
        self.client.event(Pressure {
            self_id: self.id,
            pressure,
        });
    }

    pub fn send_distance(&self, distance: u32) {
        self.client.event(Distance {
            self_id: self.id,
            distance,
        });
    }

    pub fn send_tilt(&self, tilt_x: Fixed, tilt_y: Fixed) {
        self.client.event(Tilt {
            self_id: self.id,
            tilt_x,
            tilt_y,
        });
    }

    pub fn send_rotation(&self, degrees: Fixed) {
        self.client.event(Rotation {
            self_id: self.id,
            degrees,
        });
    }

    pub fn send_slider(&self, position: i32) {
        self.client.event(Slider {
            self_id: self.id,
            position,
        });
    }

    pub fn send_wheel(&self, degrees: Fixed, clicks: i32) {
        self.client.event(Wheel {
            self_id: self.id,
            degrees,
            clicks,
        });
    }

    pub fn send_button(&self, serial: u32, button: u32, state: ToolButtonState) {
        self.client.event(Button {
            self_id: self.id,
            serial,
            button,
            state: match state {
                ToolButtonState::Released => 0,
                ToolButtonState::Pressed => 1,
            },
        });
    }

    pub fn send_frame(&self, time: u32) {
        self.client.event(Frame {
            self_id: self.id,
            time,
        });
    }
}

impl ZwpTabletToolV2RequestHandler for ZwpTabletToolV2 {
    type Error = ZwpTabletToolV2Error;

    fn set_cursor(&self, req: SetCursor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(tool) = self.tool.get() else {
            return Ok(());
        };
        if !self.seat.client.valid_serial(req.serial) {
            log::warn!("Client tried to set_cursor with an invalid serial");
            return Ok(());
        }
        let mut cursor_opt = None;
        if req.surface.is_some() {
            let surface = self.seat.client.lookup(req.surface)?;
            let cursor = surface.get_cursor(&tool.cursor)?;
            cursor.set_hotspot(req.hotspot_x, req.hotspot_y);
            cursor_opt = Some(cursor as Rc<dyn Cursor>);
        }
        if tool.node.get().node_client_id() != Some(self.seat.client.id) {
            return Ok(());
        }
        tool.cursor.set(cursor_opt);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletToolV2;
    version = self.version;
}

impl Object for ZwpTabletToolV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(ZwpTabletToolV2, ZwpTabletToolV2Id, tablet_tools);

#[derive(Debug, Error)]
pub enum ZwpTabletToolV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(ZwpTabletToolV2Error, ClientError);
efrom!(ZwpTabletToolV2Error, WlSurfaceError);

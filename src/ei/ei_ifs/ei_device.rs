use {
    crate::{
        backend::{KeyState, ScrollAxis},
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::{ei_seat::EiSeat, ei_touchscreen::TouchChange},
            ei_object::{EiObject, EiVersion},
        },
        fixed::Fixed,
        ifs::wl_seat::PX_PER_SCROLL,
        leaks::Tracker,
        rect::Rect,
        scale::Scale,
        utils::{copyhashmap::CopyHashMap, syncqueue::SyncQueue},
        wire_ei::{
            EiDeviceId,
            ei_device::{
                ClientFrame, ClientStartEmulating, ClientStopEmulating, Destroyed, DeviceType,
                Done, EiDeviceRequestHandler, Interface, Paused, Region, RegionMappingId, Release,
                Resumed, ServerFrame, ServerStartEmulating,
            },
        },
    },
    linearize::LinearizeExt,
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub const EI_DEVICE_TYPE_VIRTUAL: u32 = 1;
#[expect(dead_code)]
pub const EI_DEVICE_TYPE_PHYSICAL: u32 = 2;

pub const REGION_MAPPING_ID_SINCE: EiVersion = EiVersion(2);

pub struct EiDevice {
    pub id: EiDeviceId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub seat: Rc<EiSeat>,

    pub button_changes: SyncQueue<(u32, KeyState)>,
    pub touch_changes: CopyHashMap<u32, TouchChange>,
    pub scroll_px: [Cell<Option<f32>>; 2],
    pub scroll_v120: [Cell<Option<i32>>; 2],
    pub scroll_stop: [Cell<Option<bool>>; 2],
    pub absolute_motion: Cell<Option<(f32, f32)>>,
    pub relative_motion: Cell<Option<(f32, f32)>>,
    pub key_changes: SyncQueue<(u32, KeyState)>,
}

pub trait EiDeviceInterface: EiObject {
    fn new(device: &Rc<EiDevice>, version: EiVersion) -> Rc<Self>;

    fn destroy(&self) -> Result<(), EiClientError>;

    fn send_destroyed(&self, serial: u32);
}

impl EiDevice {
    pub fn send_interface<T>(&self, interface: &T)
    where
        T: EiDeviceInterface,
    {
        self.client.event(Interface {
            self_id: self.id,
            object: interface.id(),
            interface_name: interface.interface().name(),
            version: interface.version().0,
        });
    }

    pub fn send_device_type(&self, ty: u32) {
        self.client.event(DeviceType {
            self_id: self.id,
            device_type: ty,
        });
    }

    pub fn send_resumed(&self, serial: u32) {
        self.client.event(Resumed {
            self_id: self.id,
            serial,
        });
    }

    pub fn send_start_emulating(&self, serial: u32, sequence: u32) {
        self.client.event(ServerStartEmulating {
            self_id: self.id,
            serial,
            sequence,
        });
    }

    pub fn send_region(&self, rect: Rect, scale: Scale) {
        self.client.event(Region {
            self_id: self.id,
            offset_x: rect.x1() as u32,
            offset_y: rect.y1() as u32,
            width: rect.width() as u32,
            hight: rect.height() as u32,
            scale: (1.0 / scale.to_f64()) as f32,
        });
    }

    pub fn send_region_mapping_id(&self, mapping_id: &str) {
        if self.version >= REGION_MAPPING_ID_SINCE {
            self.client.event(RegionMappingId {
                self_id: self.id,
                mapping_id,
            });
        }
    }

    #[expect(dead_code)]
    pub fn send_paused(&self, serial: u32) {
        self.client.event(Paused {
            self_id: self.id,
            serial,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_frame(&self, serial: u32, timestamp: u64) {
        self.client.event(ServerFrame {
            self_id: self.id,
            serial,
            timestamp,
        });
    }

    pub fn send_destroyed(&self, serial: u32) {
        self.client.event(Destroyed {
            self_id: self.id,
            serial,
        });
    }

    pub fn destroy(&self) -> Result<(), EiClientError> {
        macro_rules! destroy_interface {
            ($name:ident) => {
                if let Some(interface) = self.seat.$name.take() {
                    interface.destroy()?;
                }
            };
        }
        destroy_interface!(pointer);
        destroy_interface!(pointer_absolute);
        destroy_interface!(scroll);
        destroy_interface!(button);
        destroy_interface!(keyboard);
        destroy_interface!(touchscreen);
        self.send_destroyed(self.client.serial());
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl EiDeviceRequestHandler for EiDevice {
    type Error = EiDeviceError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_start_emulating(
        &self,
        _req: ClientStartEmulating,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn client_stop_emulating(
        &self,
        _req: ClientStopEmulating,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn client_frame(&self, req: ClientFrame, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = &self.seat.seat;
        let time = req.timestamp;
        while let Some((button, pressed)) = self.button_changes.pop() {
            seat.button_event(time, button, pressed);
        }
        while let Some((button, pressed)) = self.key_changes.pop() {
            let phy = seat.get_physical_keyboard(self.seat.keyboard_id, None);
            phy.phy_state.update(time, seat, button, pressed);
        }
        if let Some((x, y)) = self.relative_motion.take() {
            let x = Fixed::from_f32(x);
            let y = Fixed::from_f32(y);
            seat.motion_event(time, x, y, x, y);
        }
        if let Some((x, y)) = self.absolute_motion.take() {
            let x = Fixed::from_f32(x);
            let y = Fixed::from_f32(y);
            seat.motion_event_abs(time, x, y);
        }
        {
            let mut need_frame = false;
            for axis in ScrollAxis::variants() {
                let idx = axis as usize;
                if let Some(v120) = self.scroll_v120[idx].take() {
                    need_frame = true;
                    seat.axis_120(v120, axis, false);
                }
                if let Some(px) = self.scroll_px[idx].take() {
                    need_frame = true;
                    seat.axis_px(Fixed::from_f32(px), axis, false);
                }
                if self.scroll_stop[idx].take() == Some(true) {
                    need_frame = true;
                    seat.axis_stop(axis);
                }
            }
            if need_frame {
                seat.axis_frame(PX_PER_SCROLL, time);
            }
        }
        if self.touch_changes.is_not_empty() {
            for (touch_id, change) in self.touch_changes.lock().drain() {
                let id = touch_id as i32;
                match change {
                    TouchChange::Down(x, y) => {
                        let x = Fixed::from_f32(x);
                        let y = Fixed::from_f32(y);
                        seat.touch_down_at(time, id, x, y);
                    }
                    TouchChange::Motion(x, y) => {
                        let x = Fixed::from_f32(x);
                        let y = Fixed::from_f32(y);
                        seat.touch_motion_at(time, id, x, y);
                    }
                    TouchChange::Up => seat.touch_up(time, id),
                    TouchChange::Cancel => seat.touch_cancel(time, id),
                }
            }
            seat.touch_frame(time);
        }
        Ok(())
    }
}

ei_object_base! {
    self = EiDevice;
    version = self.version;
}

impl EiObject for EiDevice {}

#[derive(Debug, Error)]
pub enum EiDeviceError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiDeviceError, EiClientError);

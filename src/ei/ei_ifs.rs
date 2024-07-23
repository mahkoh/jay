macro_rules! ei_device_interface {
    ($camel:ident, $snake:ident, $field:ident) => {
        impl EiDeviceInterface for $camel {
            fn new(device: &Rc<EiDevice>, version: EiVersion) -> Rc<Self> {
                let v = Rc::new(Self {
                    id: device.client.new_id(),
                    client: device.client.clone(),
                    tracker: Default::default(),
                    version,
                    device: device.clone(),
                });
                track!(v.client, v);
                v
            }

            fn destroy(&self) -> Result<(), EiClientError> {
                self.send_destroyed(self.client.serial());
                self.client.remove_obj(self)?;
                self.device.seat.$field.take();
                Ok(())
            }

            fn send_destroyed(&self, serial: u32) {
                self.client.event(crate::wire_ei::$snake::Destroyed {
                    self_id: self.id,
                    serial,
                });
            }
        }
    };
}

pub mod ei_button;
pub mod ei_callback;
pub mod ei_connection;
pub mod ei_device;
pub mod ei_handshake;
pub mod ei_keyboard;
pub mod ei_pingpong;
pub mod ei_pointer;
pub mod ei_pointer_absolute;
pub mod ei_scroll;
pub mod ei_seat;
pub mod ei_touchscreen;

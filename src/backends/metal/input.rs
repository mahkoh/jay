use {
    crate::{
        backend::{AxisSource, InputEvent, KeyState, ScrollAxis},
        backends::metal::MetalBackend,
        fixed::Fixed,
        libinput::{
            consts::{
                LIBINPUT_BUTTON_STATE_PRESSED, LIBINPUT_KEY_STATE_PRESSED,
                LIBINPUT_POINTER_AXIS_SCROLL_HORIZONTAL, LIBINPUT_POINTER_AXIS_SCROLL_VERTICAL,
            },
            event::LibInputEvent,
        },
        utils::{bitflags::BitflagsExt, errorfmt::ErrorFmt},
    },
    std::rc::Rc,
    uapi::c,
};

macro_rules! unpack {
    ($slf:expr, $ev:expr) => {{
        let slot = match $ev.device().slot() {
            Some(s) => s,
            _ => return,
        };
        let data = match $slf
            .device_holder
            .input_devices
            .borrow_mut()
            .get(slot)
            .cloned()
            .and_then(|v| v)
        {
            Some(d) => d,
            _ => return,
        };
        data
    }};
    ($slf:expr, $ev:expr, $conv:ident) => {{
        let event = match $ev.$conv() {
            Some(e) => e,
            _ => return,
        };
        let data = unpack!($slf, $ev);
        (event, data)
    }};
}

impl MetalBackend {
    pub async fn handle_libinput_events(self: Rc<Self>) {
        loop {
            match self.state.ring.readable(&self.libinput_fd).await {
                Err(e) => {
                    log::error!(
                        "Cannot wait for libinput fd to become readable: {}",
                        ErrorFmt(e)
                    );
                    break;
                }
                Ok(n) if n.intersects(c::POLLERR | c::POLLHUP) => {
                    log::error!("libinput fd fd is in an error state");
                    break;
                }
                _ => {}
            }
            if let Err(e) = self.libinput.dispatch() {
                log::error!("Could not dispatch libinput events: {}", ErrorFmt(e));
                break;
            }
            while let Some(event) = self.libinput.event() {
                self.handle_event(event);
            }
        }
        log::error!("Libinput task exited. Future input events will be ignored.");
    }

    fn handle_event(self: &Rc<Self>, event: LibInputEvent) {
        use crate::libinput::consts as c;

        match event.ty() {
            c::LIBINPUT_EVENT_DEVICE_ADDED => self.handle_device_added(event),
            c::LIBINPUT_EVENT_DEVICE_REMOVED => self.handle_li_device_removed(event),
            c::LIBINPUT_EVENT_KEYBOARD_KEY => self.handle_keyboard_key(event),
            c::LIBINPUT_EVENT_POINTER_MOTION => self.handle_pointer_motion(event),
            c::LIBINPUT_EVENT_POINTER_BUTTON => self.handle_pointer_button(event),
            c::LIBINPUT_EVENT_POINTER_SCROLL_WHEEL => {
                self.handle_pointer_axis(event, AxisSource::Wheel)
            }
            c::LIBINPUT_EVENT_POINTER_SCROLL_FINGER => {
                self.handle_pointer_axis(event, AxisSource::Finger)
            }
            c::LIBINPUT_EVENT_POINTER_SCROLL_CONTINUOUS => {
                self.handle_pointer_axis(event, AxisSource::Continuous)
            }
            _ => {}
        }
    }

    fn handle_device_added(self: &Rc<Self>, _event: LibInputEvent) {
        // let dev = unpack!(self, event);
    }

    fn handle_li_device_removed(self: &Rc<Self>, event: LibInputEvent) {
        let dev = unpack!(self, event);
        dev.inputdev.set(None);
        event.device().unset_slot();
    }

    fn handle_keyboard_key(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, keyboard_event);
        let state = if event.key_state() == LIBINPUT_KEY_STATE_PRESSED {
            if dev.pressed_keys.insert(event.key(), ()).is_some() {
                return;
            }
            KeyState::Pressed
        } else {
            if dev.pressed_keys.remove(&event.key()).is_none() {
                return;
            }
            KeyState::Released
        };
        dev.event(InputEvent::Key {
            time_usec: event.time_usec(),
            key: event.key(),
            state,
        });
    }

    fn handle_pointer_axis(self: &Rc<Self>, event: LibInputEvent, source: AxisSource) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let axes = [
            (
                LIBINPUT_POINTER_AXIS_SCROLL_HORIZONTAL,
                ScrollAxis::Horizontal,
            ),
            (LIBINPUT_POINTER_AXIS_SCROLL_VERTICAL, ScrollAxis::Vertical),
        ];
        dev.event(InputEvent::AxisSource { source });
        for (pointer_axis, axis) in axes {
            if !event.has_axis(pointer_axis) {
                continue;
            }
            let scroll = match source {
                AxisSource::Wheel => event.scroll_value_v120(pointer_axis),
                _ => event.scroll_value(pointer_axis),
            };
            let ie = if scroll == 0.0 {
                InputEvent::AxisStop { axis }
            } else if source == AxisSource::Wheel {
                InputEvent::Axis120 {
                    dist: scroll as _,
                    axis,
                    inverted: dev.natural_scrolling.get(),
                }
            } else {
                InputEvent::AxisPx {
                    dist: Fixed::from_f64(scroll),
                    axis,
                    inverted: dev.natural_scrolling.get(),
                }
            };
            dev.event(ie);
        }
        dev.event(InputEvent::AxisFrame {
            time_usec: event.time_usec(),
        });
    }

    fn handle_pointer_button(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let state = if event.button_state() == LIBINPUT_BUTTON_STATE_PRESSED {
            if dev.pressed_buttons.insert(event.button(), ()).is_some() {
                return;
            }
            KeyState::Pressed
        } else {
            if dev.pressed_buttons.remove(&event.button()).is_none() {
                return;
            }
            KeyState::Released
        };
        dev.event(InputEvent::Button {
            time_usec: event.time_usec(),
            button: event.button(),
            state,
        });
    }

    fn handle_pointer_motion(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let mut dx = event.dx();
        let mut dy = event.dy();
        let mut dx_unaccelerated = event.dx_unaccelerated();
        let mut dy_unaccelerated = event.dy_unaccelerated();
        if let Some(matrix) = dev.transform_matrix.get() {
            (dx, dy) = (
                matrix[0][0] * dx + matrix[0][1] * dy,
                matrix[1][0] * dx + matrix[1][1] * dy,
            );
            (dx_unaccelerated, dy_unaccelerated) = (
                matrix[0][0] * dx_unaccelerated + matrix[0][1] * dy_unaccelerated,
                matrix[1][0] * dx_unaccelerated + matrix[1][1] * dy_unaccelerated,
            );
        }
        dev.event(InputEvent::Motion {
            time_usec: event.time_usec(),
            dx: Fixed::from_f64(dx),
            dy: Fixed::from_f64(dy),
            dx_unaccelerated: Fixed::from_f64(dx_unaccelerated),
            dy_unaccelerated: Fixed::from_f64(dy_unaccelerated),
        });
    }
}

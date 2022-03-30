use crate::async_engine::FdStatus;
use crate::backend::{InputEvent, KeyState, ScrollAxis};
use crate::backends::metal::MetalBackend;
use crate::libinput::consts::{
    LIBINPUT_BUTTON_STATE_PRESSED, LIBINPUT_KEY_STATE_PRESSED,
    LIBINPUT_POINTER_AXIS_SCROLL_HORIZONTAL, LIBINPUT_POINTER_AXIS_SCROLL_VERTICAL,
};
use crate::libinput::event::LibInputEvent;
use crate::utils::errorfmt::ErrorFmt;
use std::rc::Rc;

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
            match self.libinput_fd.readable().await {
                Err(e) => {
                    log::error!(
                        "Cannot wait for libinput fd to become readable: {}",
                        ErrorFmt(e)
                    );
                    break;
                }
                Ok(FdStatus::Err) => {
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
            c::LIBINPUT_EVENT_POINTER_SCROLL_WHEEL => self.handle_pointer_scroll_wheel(event),
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
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        dev.event(InputEvent::Key(event.key(), state));
    }

    fn handle_pointer_scroll_wheel(self: &Rc<Self>, event: LibInputEvent) {
        const PX_PER_SCROLL: f64 = 15.0;
        const ONE_TWENTRY: f64 = 120.0;
        let (event, dev) = unpack!(self, event, pointer_event);
        let axes = [
            (
                LIBINPUT_POINTER_AXIS_SCROLL_HORIZONTAL,
                &dev.hscroll,
                ScrollAxis::Horizontal,
            ),
            (
                LIBINPUT_POINTER_AXIS_SCROLL_VERTICAL,
                &dev.vscroll,
                ScrollAxis::Vertical,
            ),
        ];
        for (axis, val, sa) in axes {
            if !event.has_axis(axis) {
                continue;
            }
            let scroll = event.scroll_value_v120(axis) / ONE_TWENTRY + val.get();
            let scroll_used = (PX_PER_SCROLL * scroll).round();
            val.set(scroll - scroll_used / PX_PER_SCROLL);
            if scroll_used != 0.0 {
                dev.event(InputEvent::Scroll(scroll_used as i32, sa));
            }
        }
    }

    fn handle_pointer_button(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let state = if event.button_state() == LIBINPUT_BUTTON_STATE_PRESSED {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        dev.event(InputEvent::Button(event.button(), state));
    }

    fn handle_pointer_motion(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let mut dx = event.dx();
        let mut dy = event.dy();
        if let Some(matrix) = dev.transform_matrix.get() {
            dx = matrix[0][0] * dx + matrix[0][1] * dy;
            dy = matrix[1][0] * dx + matrix[1][1] * dy;
        }
        dev.event(InputEvent::Motion(dx.into(), dy.into()));
    }
}

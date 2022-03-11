use crate::libinput::consts::{ButtonState, EventType, KeyState, PointerAxis};
use crate::libinput::device::LibInputDevice;
use crate::libinput::sys::{libinput_event, libinput_event_destroy, libinput_event_get_device, libinput_event_get_keyboard_event, libinput_event_get_pointer_event, libinput_event_get_type, libinput_event_keyboard, libinput_event_keyboard_get_key, libinput_event_keyboard_get_key_state, libinput_event_keyboard_get_time_usec, libinput_event_pointer, libinput_event_pointer_get_button, libinput_event_pointer_get_button_state, libinput_event_pointer_get_dx, libinput_event_pointer_get_dy, libinput_event_pointer_get_scroll_value_v120, libinput_event_pointer_get_time_usec};
use std::marker::PhantomData;

pub struct LibInputEvent<'a> {
    pub(super) event: *mut libinput_event,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputEventKeyboard<'a> {
    pub(super) event: *mut libinput_event_keyboard,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputEventPointer<'a> {
    pub(super) event: *mut libinput_event_pointer,
    pub(super) _phantom: PhantomData<&'a ()>,
}

impl<'a> Drop for LibInputEvent<'a> {
    fn drop(&mut self) {
        unsafe {
            libinput_event_destroy(self.event);
        }
    }
}

impl<'a> LibInputEvent<'a> {
    pub fn ty(&self) -> EventType {
        unsafe { EventType(libinput_event_get_type(self.event)) }
    }

    pub fn device(&self) -> LibInputDevice {
        LibInputDevice {
            dev: unsafe { libinput_event_get_device(self.event) },
            _phantom: Default::default(),
        }
    }

    pub fn keyboard_event(&self) -> Option<LibInputEventKeyboard> {
        let res = unsafe { libinput_event_get_keyboard_event(self.event) };
        if res.is_null() {
            None
        } else {
            Some(LibInputEventKeyboard {
                event: res,
                _phantom: Default::default(),
            })
        }
    }

    pub fn pointer_event(&self) -> Option<LibInputEventPointer> {
        let res = unsafe { libinput_event_get_pointer_event(self.event) };
        if res.is_null() {
            None
        } else {
            Some(LibInputEventPointer {
                event: res,
                _phantom: Default::default(),
            })
        }
    }
}

impl<'a> LibInputEventKeyboard<'a> {
    pub fn key(&self) -> u32 {
        unsafe { libinput_event_keyboard_get_key(self.event) }
    }

    pub fn key_state(&self) -> KeyState {
        unsafe { KeyState(libinput_event_keyboard_get_key_state(self.event)) }
    }

    #[allow(dead_code)]
    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_keyboard_get_time_usec(self.event) }
    }
}

impl<'a> LibInputEventPointer<'a> {
    pub fn dx(&self) -> f64 {
        unsafe { libinput_event_pointer_get_dx(self.event) }
    }

    pub fn dy(&self) -> f64 {
        unsafe { libinput_event_pointer_get_dy(self.event) }
    }

    pub fn button(&self) -> u32 {
        unsafe { libinput_event_pointer_get_button(self.event) }
    }

    pub fn button_state(&self) -> ButtonState {
        unsafe { ButtonState(libinput_event_pointer_get_button_state(self.event)) }
    }

    pub fn scroll_value_v120(&self, axis: PointerAxis) -> f64 {
        unsafe { libinput_event_pointer_get_scroll_value_v120(self.event, axis.raw() as _) }
    }

    #[allow(dead_code)]
    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_pointer_get_time_usec(self.event) }
    }
}

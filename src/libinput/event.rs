use {
    crate::libinput::{
        consts::{ButtonState, EventType, KeyState, PointerAxis, Switch, SwitchState},
        device::LibInputDevice,
        sys::{
            libinput_event, libinput_event_destroy, libinput_event_gesture,
            libinput_event_gesture_get_angle_delta, libinput_event_gesture_get_cancelled,
            libinput_event_gesture_get_dx, libinput_event_gesture_get_dx_unaccelerated,
            libinput_event_gesture_get_dy, libinput_event_gesture_get_dy_unaccelerated,
            libinput_event_gesture_get_finger_count, libinput_event_gesture_get_scale,
            libinput_event_gesture_get_time_usec, libinput_event_get_device,
            libinput_event_get_gesture_event, libinput_event_get_keyboard_event,
            libinput_event_get_pointer_event, libinput_event_get_switch_event,
            libinput_event_get_type, libinput_event_keyboard, libinput_event_keyboard_get_key,
            libinput_event_keyboard_get_key_state, libinput_event_keyboard_get_time_usec,
            libinput_event_pointer, libinput_event_pointer_get_button,
            libinput_event_pointer_get_button_state, libinput_event_pointer_get_dx,
            libinput_event_pointer_get_dx_unaccelerated, libinput_event_pointer_get_dy,
            libinput_event_pointer_get_dy_unaccelerated, libinput_event_pointer_get_scroll_value,
            libinput_event_pointer_get_scroll_value_v120, libinput_event_pointer_get_time_usec,
            libinput_event_pointer_has_axis, libinput_event_switch,
            libinput_event_switch_get_switch, libinput_event_switch_get_switch_state,
            libinput_event_switch_get_time_usec,
        },
    },
    std::marker::PhantomData,
};

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

pub struct LibInputEventGesture<'a> {
    pub(super) event: *mut libinput_event_gesture,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputEventSwitch<'a> {
    pub(super) event: *mut libinput_event_switch,
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

    pub fn gesture_event(&self) -> Option<LibInputEventGesture> {
        let res = unsafe { libinput_event_get_gesture_event(self.event) };
        if res.is_null() {
            None
        } else {
            Some(LibInputEventGesture {
                event: res,
                _phantom: Default::default(),
            })
        }
    }

    pub fn switch_event(&self) -> Option<LibInputEventSwitch> {
        let res = unsafe { libinput_event_get_switch_event(self.event) };
        if res.is_null() {
            None
        } else {
            Some(LibInputEventSwitch {
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

    pub fn dx_unaccelerated(&self) -> f64 {
        unsafe { libinput_event_pointer_get_dx_unaccelerated(self.event) }
    }

    pub fn dy_unaccelerated(&self) -> f64 {
        unsafe { libinput_event_pointer_get_dy_unaccelerated(self.event) }
    }

    pub fn button(&self) -> u32 {
        unsafe { libinput_event_pointer_get_button(self.event) }
    }

    pub fn button_state(&self) -> ButtonState {
        unsafe { ButtonState(libinput_event_pointer_get_button_state(self.event)) }
    }

    pub fn scroll_value(&self, axis: PointerAxis) -> f64 {
        unsafe { libinput_event_pointer_get_scroll_value(self.event, axis.raw() as _) }
    }

    pub fn scroll_value_v120(&self, axis: PointerAxis) -> f64 {
        unsafe { libinput_event_pointer_get_scroll_value_v120(self.event, axis.raw() as _) }
    }

    pub fn has_axis(&self, axis: PointerAxis) -> bool {
        unsafe { libinput_event_pointer_has_axis(self.event, axis.raw() as _) != 0 }
    }

    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_pointer_get_time_usec(self.event) }
    }
}

impl<'a> LibInputEventGesture<'a> {
    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_gesture_get_time_usec(self.event) }
    }

    pub fn finger_count(&self) -> u32 {
        unsafe { libinput_event_gesture_get_finger_count(self.event) as u32 }
    }

    pub fn cancelled(&self) -> bool {
        unsafe { libinput_event_gesture_get_cancelled(self.event) != 0 }
    }

    pub fn dx(&self) -> f64 {
        unsafe { libinput_event_gesture_get_dx(self.event) }
    }

    pub fn dy(&self) -> f64 {
        unsafe { libinput_event_gesture_get_dy(self.event) }
    }

    pub fn dx_unaccelerated(&self) -> f64 {
        unsafe { libinput_event_gesture_get_dx_unaccelerated(self.event) }
    }

    pub fn dy_unaccelerated(&self) -> f64 {
        unsafe { libinput_event_gesture_get_dy_unaccelerated(self.event) }
    }

    pub fn scale(&self) -> f64 {
        unsafe { libinput_event_gesture_get_scale(self.event) }
    }

    pub fn angle_delta(&self) -> f64 {
        unsafe { libinput_event_gesture_get_angle_delta(self.event) }
    }
}

impl<'a> LibInputEventSwitch<'a> {
    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_switch_get_time_usec(self.event) }
    }

    pub fn switch(&self) -> Switch {
        unsafe { Switch(libinput_event_switch_get_switch(self.event)) }
    }

    pub fn switch_state(&self) -> SwitchState {
        unsafe { SwitchState(libinput_event_switch_get_switch_state(self.event)) }
    }
}

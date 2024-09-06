use {
    crate::libinput::{
        consts::{
            ButtonState, EventType, KeyState, PointerAxis, Switch, SwitchState,
            TabletPadRingAxisSource, TabletPadStripAxisSource, TabletToolProximityState,
            TabletToolTipState, TabletToolType,
        },
        device::{LibInputDevice, LibInputTabletPadModeGroup},
        sys::{
            libinput_event, libinput_event_destroy, libinput_event_gesture,
            libinput_event_gesture_get_angle_delta, libinput_event_gesture_get_cancelled,
            libinput_event_gesture_get_dx, libinput_event_gesture_get_dx_unaccelerated,
            libinput_event_gesture_get_dy, libinput_event_gesture_get_dy_unaccelerated,
            libinput_event_gesture_get_finger_count, libinput_event_gesture_get_scale,
            libinput_event_gesture_get_time_usec, libinput_event_get_device,
            libinput_event_get_gesture_event, libinput_event_get_keyboard_event,
            libinput_event_get_pointer_event, libinput_event_get_switch_event,
            libinput_event_get_tablet_pad_event, libinput_event_get_tablet_tool_event,
            libinput_event_get_touch_event, libinput_event_get_type, libinput_event_keyboard,
            libinput_event_keyboard_get_key, libinput_event_keyboard_get_key_state,
            libinput_event_keyboard_get_time_usec, libinput_event_pointer,
            libinput_event_pointer_get_button, libinput_event_pointer_get_button_state,
            libinput_event_pointer_get_dx, libinput_event_pointer_get_dx_unaccelerated,
            libinput_event_pointer_get_dy, libinput_event_pointer_get_dy_unaccelerated,
            libinput_event_pointer_get_scroll_value, libinput_event_pointer_get_scroll_value_v120,
            libinput_event_pointer_get_time_usec, libinput_event_pointer_has_axis,
            libinput_event_switch, libinput_event_switch_get_switch,
            libinput_event_switch_get_switch_state, libinput_event_switch_get_time_usec,
            libinput_event_tablet_pad, libinput_event_tablet_pad_get_button_number,
            libinput_event_tablet_pad_get_button_state, libinput_event_tablet_pad_get_mode,
            libinput_event_tablet_pad_get_mode_group, libinput_event_tablet_pad_get_ring_number,
            libinput_event_tablet_pad_get_ring_position, libinput_event_tablet_pad_get_ring_source,
            libinput_event_tablet_pad_get_strip_number,
            libinput_event_tablet_pad_get_strip_position,
            libinput_event_tablet_pad_get_strip_source, libinput_event_tablet_pad_get_time_usec,
            libinput_event_tablet_tool, libinput_event_tablet_tool_get_button,
            libinput_event_tablet_tool_get_button_state,
            libinput_event_tablet_tool_get_proximity_state,
            libinput_event_tablet_tool_get_time_usec, libinput_event_tablet_tool_get_tip_state,
            libinput_event_tablet_tool_get_tool,
            libinput_event_tablet_tool_get_wheel_delta_discrete,
            libinput_event_tablet_tool_get_x_transformed,
            libinput_event_tablet_tool_get_y_transformed, libinput_event_touch,
            libinput_event_touch_get_seat_slot, libinput_event_touch_get_time_usec,
            libinput_event_touch_get_x_transformed, libinput_event_touch_get_y_transformed,
            libinput_tablet_tool, libinput_tablet_tool_get_serial,
            libinput_tablet_tool_get_tool_id, libinput_tablet_tool_get_type,
            libinput_tablet_tool_get_user_data, libinput_tablet_tool_set_user_data,
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

pub struct LibInputEventTabletTool<'a> {
    pub(super) event: *mut libinput_event_tablet_tool,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputEventTabletPad<'a> {
    pub(super) event: *mut libinput_event_tablet_pad,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputTabletTool<'a> {
    pub(super) tool: *mut libinput_tablet_tool,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputEventTouch<'a> {
    pub(super) event: *mut libinput_event_touch,
    pub(super) _phantom: PhantomData<&'a ()>,
}

impl<'a> Drop for LibInputEvent<'a> {
    fn drop(&mut self) {
        unsafe {
            libinput_event_destroy(self.event);
        }
    }
}

macro_rules! converter {
    ($name:ident, $out:ident, $f:ident) => {
        pub fn $name(&self) -> Option<$out> {
            let res = unsafe { $f(self.event) };
            if res.is_null() {
                None
            } else {
                Some($out {
                    event: res,
                    _phantom: Default::default(),
                })
            }
        }
    };
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

    converter!(
        keyboard_event,
        LibInputEventKeyboard,
        libinput_event_get_keyboard_event
    );
    converter!(
        pointer_event,
        LibInputEventPointer,
        libinput_event_get_pointer_event
    );
    converter!(
        gesture_event,
        LibInputEventGesture,
        libinput_event_get_gesture_event
    );
    converter!(
        switch_event,
        LibInputEventSwitch,
        libinput_event_get_switch_event
    );
    converter!(
        tablet_tool_event,
        LibInputEventTabletTool,
        libinput_event_get_tablet_tool_event
    );
    converter!(
        tablet_pad_event,
        LibInputEventTabletPad,
        libinput_event_get_tablet_pad_event
    );
    converter!(
        touch_event,
        LibInputEventTouch,
        libinput_event_get_touch_event
    );
}

impl<'a> LibInputEventKeyboard<'a> {
    pub fn key(&self) -> u32 {
        unsafe { libinput_event_keyboard_get_key(self.event) }
    }

    pub fn key_state(&self) -> KeyState {
        unsafe { KeyState(libinput_event_keyboard_get_key_state(self.event)) }
    }

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

macro_rules! has_changed {
    ($name:ident, $f:ident) => {
        pub fn $name(&self) -> bool {
            unsafe { crate::libinput::sys::$f(self.event) != 0 }
        }
    };
}

macro_rules! get_double {
    ($name:ident, $f:ident) => {
        pub fn $name(&self) -> f64 {
            unsafe { crate::libinput::sys::$f(self.event) }
        }
    };
}

macro_rules! has_capability {
    ($name:ident, $f:ident) => {
        pub fn $name(&self) -> bool {
            unsafe { crate::libinput::sys::$f(self.tool) != 0 }
        }
    };
}

impl<'a> LibInputTabletTool<'a> {
    pub fn user_data(&self) -> usize {
        unsafe { libinput_tablet_tool_get_user_data(self.tool) }
    }

    pub fn set_user_data(&self, user_data: usize) {
        unsafe { libinput_tablet_tool_set_user_data(self.tool, user_data) }
    }

    pub fn type_(&self) -> TabletToolType {
        unsafe { TabletToolType(libinput_tablet_tool_get_type(self.tool)) }
    }

    pub fn tool_id(&self) -> u64 {
        unsafe { libinput_tablet_tool_get_tool_id(self.tool) }
    }

    pub fn serial(&self) -> u64 {
        unsafe { libinput_tablet_tool_get_serial(self.tool) }
    }

    has_capability!(has_pressure, libinput_tablet_tool_has_pressure);
    has_capability!(has_distance, libinput_tablet_tool_has_distance);
    has_capability!(has_tilt, libinput_tablet_tool_has_tilt);
    has_capability!(has_rotation, libinput_tablet_tool_has_rotation);
    has_capability!(has_slider, libinput_tablet_tool_has_slider);
    // has_capability!(has_size, libinput_tablet_tool_has_size);
    has_capability!(has_wheel, libinput_tablet_tool_has_wheel);
}

impl<'a> LibInputEventTabletTool<'a> {
    pub fn tool(&self) -> LibInputTabletTool<'_> {
        LibInputTabletTool {
            tool: unsafe { libinput_event_tablet_tool_get_tool(self.event) },
            _phantom: Default::default(),
        }
    }

    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_tablet_tool_get_time_usec(self.event) }
    }

    has_changed!(x_has_changed, libinput_event_tablet_tool_x_has_changed);
    has_changed!(y_has_changed, libinput_event_tablet_tool_y_has_changed);
    has_changed!(
        pressure_has_changed,
        libinput_event_tablet_tool_pressure_has_changed
    );
    has_changed!(
        distance_has_changed,
        libinput_event_tablet_tool_distance_has_changed
    );
    has_changed!(
        tilt_x_has_changed,
        libinput_event_tablet_tool_tilt_x_has_changed
    );
    has_changed!(
        tilt_y_has_changed,
        libinput_event_tablet_tool_tilt_y_has_changed
    );
    has_changed!(
        rotation_has_changed,
        libinput_event_tablet_tool_rotation_has_changed
    );
    has_changed!(
        slider_has_changed,
        libinput_event_tablet_tool_slider_has_changed
    );
    // has_changed!(
    //     size_major_has_changed,
    //     libinput_event_tablet_tool_size_major_has_changed
    // );
    // has_changed!(
    //     size_minor_has_changed,
    //     libinput_event_tablet_tool_size_minor_has_changed
    // );
    has_changed!(
        wheel_has_changed,
        libinput_event_tablet_tool_wheel_has_changed
    );

    // get_double!(x, libinput_event_tablet_tool_get_x);
    // get_double!(y, libinput_event_tablet_tool_get_y);
    get_double!(dx, libinput_event_tablet_tool_get_dx);
    get_double!(dy, libinput_event_tablet_tool_get_dy);
    get_double!(pressure, libinput_event_tablet_tool_get_pressure);
    get_double!(distance, libinput_event_tablet_tool_get_distance);
    get_double!(tilt_x, libinput_event_tablet_tool_get_tilt_x);
    get_double!(tilt_y, libinput_event_tablet_tool_get_tilt_y);
    get_double!(rotation, libinput_event_tablet_tool_get_rotation);
    get_double!(
        slider_position,
        libinput_event_tablet_tool_get_slider_position
    );
    // get_double!(size_major, libinput_event_tablet_tool_get_size_major);
    // get_double!(size_minor, libinput_event_tablet_tool_get_size_minor);
    get_double!(wheel_delta, libinput_event_tablet_tool_get_wheel_delta);

    pub fn wheel_delta_discrete(&self) -> i32 {
        unsafe { libinput_event_tablet_tool_get_wheel_delta_discrete(self.event) as _ }
    }

    pub fn x_transformed(&self, width: u32) -> f64 {
        unsafe { libinput_event_tablet_tool_get_x_transformed(self.event, width) }
    }

    pub fn y_transformed(&self, width: u32) -> f64 {
        unsafe { libinput_event_tablet_tool_get_y_transformed(self.event, width) }
    }

    pub fn proximity_state(&self) -> TabletToolProximityState {
        unsafe {
            TabletToolProximityState(libinput_event_tablet_tool_get_proximity_state(self.event))
        }
    }

    pub fn tip_state(&self) -> TabletToolTipState {
        unsafe { TabletToolTipState(libinput_event_tablet_tool_get_tip_state(self.event)) }
    }

    pub fn button(&self) -> u32 {
        unsafe { libinput_event_tablet_tool_get_button(self.event) }
    }

    pub fn button_state(&self) -> ButtonState {
        unsafe { ButtonState(libinput_event_tablet_tool_get_button_state(self.event)) }
    }
}

impl<'a> LibInputEventTabletPad<'a> {
    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_tablet_pad_get_time_usec(self.event) }
    }

    pub fn ring_position(&self) -> f64 {
        unsafe { libinput_event_tablet_pad_get_ring_position(self.event) }
    }

    pub fn ring_number(&self) -> u32 {
        unsafe { libinput_event_tablet_pad_get_ring_number(self.event) as u32 }
    }

    pub fn ring_source(&self) -> TabletPadRingAxisSource {
        unsafe { TabletPadRingAxisSource(libinput_event_tablet_pad_get_ring_source(self.event)) }
    }

    pub fn strip_position(&self) -> f64 {
        unsafe { libinput_event_tablet_pad_get_strip_position(self.event) }
    }

    pub fn strip_number(&self) -> u32 {
        unsafe { libinput_event_tablet_pad_get_strip_number(self.event) as u32 }
    }

    pub fn strip_source(&self) -> TabletPadStripAxisSource {
        unsafe { TabletPadStripAxisSource(libinput_event_tablet_pad_get_strip_source(self.event)) }
    }

    pub fn button_number(&self) -> u32 {
        unsafe { libinput_event_tablet_pad_get_button_number(self.event) }
    }

    pub fn button_state(&self) -> ButtonState {
        unsafe { ButtonState(libinput_event_tablet_pad_get_button_state(self.event)) }
    }

    pub fn mode(&self) -> u32 {
        unsafe { libinput_event_tablet_pad_get_mode(self.event) as u32 }
    }

    pub fn mode_group(&self) -> LibInputTabletPadModeGroup {
        LibInputTabletPadModeGroup {
            group: unsafe { libinput_event_tablet_pad_get_mode_group(self.event) },
            _phantom: Default::default(),
        }
    }
}

impl<'a> LibInputEventTouch<'a> {
    pub fn seat_slot(&self) -> i32 {
        unsafe { libinput_event_touch_get_seat_slot(self.event) }
    }

    // pub fn x(&self) -> f64 {
    //     unsafe { libinput_event_touch_get_x(self.event) }
    // }
    //
    // pub fn y(&self) -> f64 {
    //     unsafe { libinput_event_touch_get_y(self.event) }
    // }

    pub fn x_transformed(&self, width: u32) -> f64 {
        unsafe { libinput_event_touch_get_x_transformed(self.event, width) }
    }

    pub fn y_transformed(&self, height: u32) -> f64 {
        unsafe { libinput_event_touch_get_y_transformed(self.event, height) }
    }

    pub fn time_usec(&self) -> u64 {
        unsafe { libinput_event_touch_get_time_usec(self.event) }
    }
}

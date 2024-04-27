use uapi::c;

include!(concat!(env!("OUT_DIR"), "/libinput_tys.rs"));

pub type libinput_log_handler = unsafe extern "C" fn();

#[repr(transparent)]
pub struct libinput(u8);
#[repr(transparent)]
pub struct libinput_device(u8);
#[repr(transparent)]
pub struct libinput_event(u8);
#[repr(transparent)]
pub struct libinput_event_keyboard(u8);
#[repr(transparent)]
pub struct libinput_event_pointer(u8);
#[repr(transparent)]
pub struct libinput_event_gesture(u8);

#[link(name = "input")]
extern "C" {
    pub fn libinput_log_set_handler(libinput: *mut libinput, log_handler: libinput_log_handler);
    pub fn libinput_log_set_priority(libinput: *mut libinput, priority: libinput_log_priority);
    pub fn libinput_path_create_context(
        interface: *const libinput_interface,
        user_data: *mut c::c_void,
    ) -> *mut libinput;
    pub fn libinput_unref(libinput: *mut libinput) -> *mut libinput;
    pub fn libinput_get_fd(libinput: *mut libinput) -> c::c_int;
    pub fn libinput_dispatch(libinput: *mut libinput) -> c::c_int;
    pub fn libinput_get_event(libinput: *mut libinput) -> *mut libinput_event;

    pub fn libinput_device_set_user_data(device: *mut libinput_device, user_data: *mut c::c_void);
    pub fn libinput_device_get_user_data(device: *mut libinput_device) -> *mut c::c_void;
    pub fn libinput_device_ref(device: *mut libinput_device) -> *mut libinput_device;
    pub fn libinput_device_unref(device: *mut libinput_device) -> *mut libinput_device;
    pub fn libinput_path_add_device(
        libinput: *mut libinput,
        path: *const c::c_char,
    ) -> *mut libinput_device;
    pub fn libinput_path_remove_device(device: *mut libinput_device);
    pub fn libinput_device_has_capability(
        device: *mut libinput_device,
        cap: libinput_device_capability,
    ) -> c::c_int;
    pub fn libinput_device_config_left_handed_is_available(
        device: *mut libinput_device,
    ) -> c::c_int;
    pub fn libinput_device_config_left_handed_get(device: *mut libinput_device) -> c::c_int;
    pub fn libinput_device_config_left_handed_set(
        device: *mut libinput_device,
        left_handed: c::c_int,
    ) -> libinput_config_status;
    pub fn libinput_device_config_accel_is_available(device: *mut libinput_device) -> c::c_int;
    pub fn libinput_device_config_accel_get_profile(
        device: *mut libinput_device,
    ) -> libinput_config_accel_profile;
    pub fn libinput_device_config_accel_set_profile(
        device: *mut libinput_device,
        profile: libinput_config_accel_profile,
    ) -> libinput_config_status;
    pub fn libinput_device_config_accel_get_speed(device: *mut libinput_device) -> f64;
    pub fn libinput_device_config_accel_set_speed(
        device: *mut libinput_device,
        speed: f64,
    ) -> libinput_config_status;
    pub fn libinput_device_get_name(device: *mut libinput_device) -> *const c::c_char;
    pub fn libinput_device_config_tap_get_finger_count(device: *mut libinput_device) -> c::c_int;
    pub fn libinput_device_config_tap_set_enabled(
        device: *mut libinput_device,
        enable: libinput_config_tap_state,
    ) -> libinput_config_status;
    pub fn libinput_device_config_tap_get_enabled(
        device: *mut libinput_device,
    ) -> libinput_config_tap_state;
    pub fn libinput_device_config_tap_set_drag_enabled(
        device: *mut libinput_device,
        enable: libinput_config_drag_state,
    ) -> libinput_config_status;
    pub fn libinput_device_config_tap_get_drag_enabled(
        device: *mut libinput_device,
    ) -> libinput_config_drag_state;
    pub fn libinput_device_config_tap_set_drag_lock_enabled(
        device: *mut libinput_device,
        enable: libinput_config_drag_lock_state,
    ) -> libinput_config_status;
    pub fn libinput_device_config_tap_get_drag_lock_enabled(
        device: *mut libinput_device,
    ) -> libinput_config_drag_lock_state;
    pub fn libinput_device_config_scroll_set_natural_scroll_enabled(
        device: *mut libinput_device,
        enable: c::c_int,
    ) -> libinput_config_status;
    pub fn libinput_device_config_scroll_get_natural_scroll_enabled(
        device: *mut libinput_device,
    ) -> c::c_int;
    pub fn libinput_device_config_scroll_has_natural_scroll(
        device: *mut libinput_device,
    ) -> c::c_int;

    pub fn libinput_event_destroy(event: *mut libinput_event);
    pub fn libinput_event_get_type(event: *mut libinput_event) -> libinput_event_type;
    pub fn libinput_event_get_device(event: *mut libinput_event) -> *mut libinput_device;

    pub fn libinput_event_get_keyboard_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_keyboard;
    pub fn libinput_event_keyboard_get_key(event: *mut libinput_event_keyboard) -> u32;
    pub fn libinput_event_keyboard_get_key_state(
        event: *mut libinput_event_keyboard,
    ) -> libinput_key_state;
    pub fn libinput_event_keyboard_get_time_usec(event: *mut libinput_event_keyboard) -> u64;

    pub fn libinput_event_get_pointer_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_pointer;
    pub fn libinput_event_pointer_get_time_usec(event: *mut libinput_event_pointer) -> u64;
    pub fn libinput_event_pointer_get_dx(event: *mut libinput_event_pointer) -> f64;
    pub fn libinput_event_pointer_get_dy(event: *mut libinput_event_pointer) -> f64;
    pub fn libinput_event_pointer_get_dx_unaccelerated(event: *mut libinput_event_pointer) -> f64;
    pub fn libinput_event_pointer_get_dy_unaccelerated(event: *mut libinput_event_pointer) -> f64;
    pub fn libinput_event_pointer_get_button(event: *mut libinput_event_pointer) -> u32;
    pub fn libinput_event_pointer_get_button_state(
        event: *mut libinput_event_pointer,
    ) -> libinput_button_state;
    pub fn libinput_event_pointer_get_scroll_value(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> f64;
    pub fn libinput_event_pointer_get_scroll_value_v120(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> f64;
    pub fn libinput_event_pointer_has_axis(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> c::c_int;
    // pub fn libinput_event_pointer_get_axis_source(
    //     event: *mut libinput_event_pointer,
    // ) -> libinput_pointer_axis_source;
    // pub fn libinput_event_pointer_get_axis_value_discrete(
    //     event: *mut libinput_event_pointer,
    //     axis: libinput_pointer_axis,
    // ) -> f64;

    pub fn libinput_event_get_gesture_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_gesture;
    pub fn libinput_event_gesture_get_time_usec(event: *mut libinput_event_gesture) -> u64;
    pub fn libinput_event_gesture_get_finger_count(event: *mut libinput_event_gesture) -> c::c_int;
    pub fn libinput_event_gesture_get_cancelled(event: *mut libinput_event_gesture) -> c::c_int;
    pub fn libinput_event_gesture_get_dx(event: *mut libinput_event_gesture) -> f64;
    pub fn libinput_event_gesture_get_dy(event: *mut libinput_event_gesture) -> f64;
    pub fn libinput_event_gesture_get_dx_unaccelerated(event: *mut libinput_event_gesture) -> f64;
    pub fn libinput_event_gesture_get_dy_unaccelerated(event: *mut libinput_event_gesture) -> f64;
    pub fn libinput_event_gesture_get_scale(event: *mut libinput_event_gesture) -> f64;
    pub fn libinput_event_gesture_get_angle_delta(event: *mut libinput_event_gesture) -> f64;
}

#[repr(C)]
pub struct libinput_interface {
    pub open_restricted: unsafe extern "C" fn(
        path: *const c::c_char,
        flags: c::c_int,
        user_data: *mut c::c_void,
    ) -> c::c_int,
    pub close_restricted: unsafe extern "C" fn(fd: c::c_int, user_data: *mut c::c_void),
}

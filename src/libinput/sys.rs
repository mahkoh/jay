use std::ffi::VaList;
use uapi::c;

include!(concat!(env!("OUT_DIR"), "/libinput_tys.rs"));

pub type libinput_log_handler = unsafe extern "C" fn(
    libinput: *mut libinput,
    priority: libinput_log_priority,
    format: *const c::c_char,
    args: VaList,
);

#[link(name = "input")]
extern "C" {
    pub type libinput;
    pub type libinput_device;
    pub type libinput_event;
    pub type libinput_event_keyboard;
    pub type libinput_event_pointer;

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
    pub fn libinput_device_config_left_handed_set(
        device: *mut libinput_device,
        left_handed: c::c_int,
    ) -> libinput_config_status;
    pub fn libinput_device_config_accel_set_profile(
        device: *mut libinput_device,
        profile: libinput_config_accel_profile,
    ) -> libinput_config_status;
    pub fn libinput_device_config_accel_set_speed(
        device: *mut libinput_device,
        speed: f64,
    ) -> libinput_config_status;
    pub fn libinput_device_get_name(device: *mut libinput_device) -> *const c::c_char;

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
    pub fn libinput_event_pointer_get_button(event: *mut libinput_event_pointer) -> u32;
    pub fn libinput_event_pointer_get_button_state(
        event: *mut libinput_event_pointer,
    ) -> libinput_button_state;
    pub fn libinput_event_pointer_get_scroll_value_v120(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> f64;
    pub fn libinput_event_pointer_has_axis(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> c::c_int;
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

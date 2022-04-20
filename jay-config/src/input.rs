pub mod acceleration;
pub mod capability;

use {
    crate::{
        input::{acceleration::AccelProfile, capability::Capability},
        Axis, Direction, Keymap, ModifiedKeySym, Workspace,
    },
    bincode::{Decode, Encode},
};

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct InputDevice(pub u64);

impl InputDevice {
    pub fn set_seat(self, seat: Seat) {
        get!().set_seat(self, seat)
    }

    pub fn has_capability(self, cap: Capability) -> bool {
        get!(false).has_capability(self, cap)
    }

    pub fn set_left_handed(self, left_handed: bool) {
        get!().set_left_handed(self, left_handed);
    }

    pub fn set_accel_profile(self, profile: AccelProfile) {
        get!().set_accel_profile(self, profile);
    }

    pub fn set_accel_speed(self, speed: f64) {
        get!().set_accel_speed(self, speed);
    }

    pub fn set_transform_matrix(self, matrix: [[f64; 2]; 2]) {
        get!().set_transform_matrix(self, matrix);
    }

    pub fn name(self) -> String {
        get!(String::new()).device_name(self)
    }
}

impl Seat {
    #[doc(hidden)]
    pub fn raw(self) -> u64 {
        self.0
    }

    #[doc(hidden)]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn bind<T: Into<ModifiedKeySym>, F: Fn() + 'static>(self, mod_sym: T, f: F) {
        get!().bind(self, mod_sym, f)
    }

    pub fn unbind<T: Into<ModifiedKeySym>>(self, mod_sym: T) {
        get!().unbind(self, mod_sym)
    }

    pub fn focus(self, direction: Direction) {
        get!().focus(self, direction)
    }

    pub fn move_(self, direction: Direction) {
        get!().move_(self, direction)
    }

    pub fn set_keymap(self, keymap: Keymap) {
        get!().seat_set_keymap(self, keymap)
    }

    pub fn repeat_rate(self) -> (i32, i32) {
        let mut res = (25, 250);
        (|| res = get!().seat_get_repeat_rate(self))();
        res
    }

    pub fn set_repeat_rate(self, rate: i32, delay: i32) {
        get!().seat_set_repeat_rate(self, rate, delay)
    }

    pub fn mono(self) -> bool {
        let mut res = false;
        (|| res = get!().mono(self))();
        res
    }

    pub fn set_mono(self, mono: bool) {
        get!().set_mono(self, mono)
    }

    pub fn split(self) -> Axis {
        let mut res = Axis::Horizontal;
        (|| res = get!().split(self))();
        res
    }

    pub fn set_split(self, axis: Axis) {
        get!().set_split(self, axis)
    }

    pub fn input_devices(self) -> Vec<InputDevice> {
        let mut res = vec![];
        (|| res = get!().get_input_devices(Some(self)))();
        res
    }

    pub fn create_split(self, axis: Axis) {
        get!().create_split(self, axis);
    }

    pub fn focus_parent(self) {
        get!().focus_parent(self);
    }

    pub fn close(self) {
        get!().close(self);
    }

    pub fn toggle_floating(self) {
        get!().toggle_floating(self);
    }

    pub fn show_workspace(self, workspace: Workspace) {
        get!().show_workspace(self, workspace)
    }

    pub fn toggle_fullscreen(self) {
        let c = get!();
        c.set_fullscreen(self, !c.get_fullscreen(self));
    }

    pub fn fullscreen(self) -> bool {
        get!(false).get_fullscreen(self)
    }

    pub fn set_fullscreen(self, fullscreen: bool) {
        get!().set_fullscreen(self, fullscreen)
    }
}

pub fn get_seats() -> Vec<Seat> {
    let mut res = vec![];
    (|| res = get!().seats())();
    res
}

pub fn input_devices() -> Vec<InputDevice> {
    let mut res = vec![];
    (|| res = get!().get_input_devices(None))();
    res
}

pub fn remove_all_seats() {}

pub fn create_seat(name: &str) -> Seat {
    let mut res = Seat(0);
    (|| res = get!().create_seat(name))();
    res
}

pub fn on_new_seat<F: Fn(Seat) + 'static>(f: F) {
    get!().on_new_seat(f)
}

pub fn on_new_input_device<F: Fn(InputDevice) + 'static>(f: F) {
    get!().on_new_input_device(f)
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Seat(pub u64);

impl Seat {
    pub const INVALID: Self = Self(0);

    pub fn is_invalid(self) -> bool {
        self == Self::INVALID
    }
}

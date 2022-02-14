#![feature(thread_local_const_init)]

use crate::keyboard::ModifiedKeySym;
use bincode::{Decode, Encode};
use crate::keyboard::keymap::Keymap;

#[macro_use]
mod macros;
#[doc(hidden)]
pub mod _private;
pub mod keyboard;

#[derive(Encode, Decode, Copy, Clone, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Encode, Decode, Copy, Clone, Debug)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Seat(pub u64);

impl Seat {
    pub const INVALID: Self = Self(0);

    pub fn is_invalid(self) -> bool {
        self == Self::INVALID
    }
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Keyboard(pub u64);

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Mouse(pub u64);

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum InputDevice {
    Keyboard(Keyboard),
    Mouse(Mouse),
}

impl InputDevice {
    pub fn set_seat(self, seat: Seat) {
        get!().set_seat(self, seat)
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

    pub fn set_repeat_rate(self, rate: i32, delay: i32) {
        get!().seat_set_repeat_rate(self, rate, delay)
    }

    pub fn repeat_rate(self) -> (i32, i32) {
        let mut res = (25, 250);
        (|| res = get!().seat_get_repeat_rate(self))();
        res
    }
}

pub fn get_seats() -> Vec<Seat> {
    let mut res = vec![];
    (|| res = get!().seats())();
    res
}

pub fn input_devices() -> Vec<InputDevice> {
    let mut res = vec![];
    (|| res = get!().get_input_devices())();
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

pub fn shell(shell: &str) {
    get!().shell(shell)
}

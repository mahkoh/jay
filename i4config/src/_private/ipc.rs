use crate::keyboard::keymap::Keymap;
use crate::keyboard::mods::Modifiers;
use crate::keyboard::syms::KeySym;
use crate::{Axis, Direction, InputDevice, LogLevel, Seat};
use bincode::{BorrowDecode, Decode, Encode};

#[derive(Encode, BorrowDecode, Debug)]
pub enum ServerMessage {
    Configure,
    Response {
        response: Response,
    },
    NewInputDevice {
        device: InputDevice,
    },
    DelInputDevice {
        device: InputDevice,
    },
    InvokeShortcut {
        seat: Seat,
        mods: Modifiers,
        sym: KeySym,
    },
}

#[derive(Encode, BorrowDecode, Debug)]
pub enum ClientMessage<'a> {
    Log {
        level: LogLevel,
        msg: &'a str,
        file: Option<&'a str>,
        line: Option<u32>,
    },
    CreateSeat {
        name: &'a str,
    },
    SetSeat {
        device: InputDevice,
        seat: Seat,
    },
    ParseKeymap {
        keymap: &'a str,
    },
    SeatSetKeymap {
        seat: Seat,
        keymap: Keymap,
    },
    SeatGetRepeatRate {
        seat: Seat,
    },
    SeatSetRepeatRate {
        seat: Seat,
        rate: i32,
        delay: i32,
    },
    GetSplit {
        seat: Seat,
    },
    SetSplit {
        seat: Seat,
        axis: Axis,
    },
    RemoveSeat {
        seat: Seat,
    },
    GetSeats,
    GetInputDevices,
    AddShortcut {
        seat: Seat,
        mods: Modifiers,
        sym: KeySym,
    },
    RemoveShortcut {
        seat: Seat,
        mods: Modifiers,
        sym: KeySym,
    },
    Run {
        prog: &'a str,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
    Focus {
        seat: Seat,
        direction: Direction,
    },
    Move {
        seat: Seat,
        direction: Direction,
    },
}

#[derive(Encode, Decode, Debug)]
pub enum Response {
    None,
    GetSeats { seats: Vec<Seat> },
    GetSplit { axis: Axis },
    GetRepeatRate { rate: i32, delay: i32 },
    ParseKeymap { keymap: Keymap },
    CreateSeat { seat: Seat },
    GetInputDevices { devices: Vec<InputDevice> },
}

#[derive(Encode, Decode, Debug)]
pub enum InitMessage {
    V1(V1InitMessage),
}

#[derive(Encode, Decode, Debug)]
pub struct V1InitMessage {}

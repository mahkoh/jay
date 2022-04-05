use crate::drm::Connector;
use crate::input::acceleration::AccelProfile;
use crate::input::capability::Capability;
use crate::input::InputDevice;
use crate::keyboard::keymap::Keymap;
use crate::keyboard::mods::Modifiers;
use crate::keyboard::syms::KeySym;
use crate::theme::Color;
use crate::{Axis, Direction, LogLevel, Seat, Workspace};
use bincode::{BorrowDecode, Decode, Encode};

#[derive(Encode, BorrowDecode, Debug)]
pub enum ServerMessage {
    Configure,
    Response {
        response: Response,
    },
    ConnectorConnect {
        device: Connector,
    },
    ConnectorDisconnect {
        device: Connector,
    },
    NewConnector {
        device: Connector,
    },
    DelConnector {
        device: Connector,
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
    Quit,
    SwitchTo {
        vtnr: u32,
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
    GetMono {
        seat: Seat,
    },
    SetMono {
        seat: Seat,
        mono: bool,
    },
    RemoveSeat {
        seat: Seat,
    },
    GetSeats,
    GetInputDevices {
        seat: Option<Seat>,
    },
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
    GrabKb {
        kb: InputDevice,
        grab: bool,
    },
    GetTitleHeight,
    GetBorderWidth,
    SetTitleHeight {
        height: i32,
    },
    SetBorderWidth {
        width: i32,
    },
    SetTitleColor {
        color: Color,
    },
    SetTitleUnderlineColor {
        color: Color,
    },
    SetBorderColor {
        color: Color,
    },
    SetBackgroundColor {
        color: Color,
    },
    CreateSplit {
        seat: Seat,
        axis: Axis,
    },
    FocusParent {
        seat: Seat,
    },
    ToggleFloating {
        seat: Seat,
    },
    HasCapability {
        device: InputDevice,
        cap: Capability,
    },
    SetLeftHanded {
        device: InputDevice,
        left_handed: bool,
    },
    SetAccelProfile {
        device: InputDevice,
        profile: AccelProfile,
    },
    SetAccelSpeed {
        device: InputDevice,
        speed: f64,
    },
    SetTransformMatrix {
        device: InputDevice,
        matrix: [[f64; 2]; 2],
    },
    GetDeviceName {
        device: InputDevice,
    },
    GetWorkspace {
        name: &'a str,
    },
    ShowWorkspace {
        seat: Seat,
        workspace: Workspace,
    },
}

#[derive(Encode, Decode, Debug)]
pub enum Response {
    None,
    GetSeats { seats: Vec<Seat> },
    GetSplit { axis: Axis },
    GetMono { mono: bool },
    GetRepeatRate { rate: i32, delay: i32 },
    ParseKeymap { keymap: Keymap },
    CreateSeat { seat: Seat },
    GetInputDevices { devices: Vec<InputDevice> },
    GetTitleHeight { height: i32 },
    GetBorderWidth { width: i32 },
    HasCapability { has: bool },
    GetDeviceName { name: String },
    GetWorkspace { workspace: Workspace },
}

#[derive(Encode, Decode, Debug)]
pub enum InitMessage {
    V1(V1InitMessage),
}

#[derive(Encode, Decode, Debug)]
pub struct V1InitMessage {}

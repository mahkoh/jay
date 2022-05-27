use {
    crate::{
        input::{acceleration::AccelProfile, capability::Capability, InputDevice, Seat},
        keyboard::{mods::Modifiers, syms::KeySym, Keymap},
        logging::LogLevel,
        theme::{colors::Colorable, sized::Resizable, Color},
        timer::Timer,
        video::{connector_type::ConnectorType, Connector, DrmDevice},
        Axis, Direction, PciId, Workspace,
    },
    bincode::{BorrowDecode, Decode, Encode},
    std::time::Duration,
};

#[derive(Encode, BorrowDecode, Debug)]
pub enum ServerMessage {
    Configure {
        reload: bool,
    },
    GraphicsInitialized,
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
    TimerExpired {
        timer: Timer,
    },
    Clear,
    NewDrmDev {
        device: DrmDevice,
    },
    DelDrmDev {
        device: DrmDevice,
    },
}

#[derive(Encode, BorrowDecode, Debug)]
pub enum ClientMessage<'a> {
    Reload,
    Quit,
    SwitchTo {
        vtnr: u32,
    },
    Log {
        level: LogLevel,
        msg: &'a str,
        file: Option<&'a str>,
        line: Option<u32>,
    },
    GetSeat {
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
    SetStatus {
        status: &'a str,
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
    ResetSizes,
    GetSize {
        sized: Resizable,
    },
    SetSize {
        sized: Resizable,
        size: i32,
    },
    ResetColors,
    GetColor {
        colorable: Colorable,
    },
    SetColor {
        colorable: Colorable,
        color: Color,
    },
    CreateSplit {
        seat: Seat,
        axis: Axis,
    },
    Close {
        seat: Seat,
    },
    FocusParent {
        seat: Seat,
    },
    GetFloating {
        seat: Seat,
    },
    SetFloating {
        seat: Seat,
        floating: bool,
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
    GetConnector {
        ty: ConnectorType,
        idx: u32,
    },
    ConnectorConnected {
        connector: Connector,
    },
    ConnectorType {
        connector: Connector,
    },
    ConnectorMode {
        connector: Connector,
    },
    ConnectorSetPosition {
        connector: Connector,
        x: i32,
        y: i32,
    },
    ShowWorkspace {
        seat: Seat,
        workspace: Workspace,
    },
    SetWorkspace {
        seat: Seat,
        workspace: Workspace,
    },
    GetTimer {
        name: &'a str,
    },
    RemoveTimer {
        timer: Timer,
    },
    ProgramTimer {
        timer: Timer,
        initial: Option<Duration>,
        periodic: Option<Duration>,
    },
    SetEnv {
        key: &'a str,
        val: &'a str,
    },
    SetFullscreen {
        seat: Seat,
        fullscreen: bool,
    },
    GetFullscreen {
        seat: Seat,
    },
    GetDeviceConnectors {
        device: DrmDevice,
    },
    GetDrmDeviceSyspath {
        device: DrmDevice,
    },
    GetDrmDeviceVendor {
        device: DrmDevice,
    },
    GetDrmDeviceModel {
        device: DrmDevice,
    },
    GetDrmDevices,
    GetDrmDevicePciId {
        device: DrmDevice,
    },
    ResetFont,
    GetFont,
    SetFont {
        font: &'a str,
    },
    SetPxPerWheelScroll {
        device: InputDevice,
        px: f64,
    },
}

#[derive(Encode, Decode, Debug)]
pub enum Response {
    None,
    GetSeats {
        seats: Vec<Seat>,
    },
    GetSplit {
        axis: Axis,
    },
    GetMono {
        mono: bool,
    },
    GetRepeatRate {
        rate: i32,
        delay: i32,
    },
    ParseKeymap {
        keymap: Keymap,
    },
    GetSeat {
        seat: Seat,
    },
    GetInputDevices {
        devices: Vec<InputDevice>,
    },
    GetSize {
        size: i32,
    },
    HasCapability {
        has: bool,
    },
    GetDeviceName {
        name: String,
    },
    GetTimer {
        timer: Timer,
    },
    GetWorkspace {
        workspace: Workspace,
    },
    GetConnector {
        connector: Connector,
    },
    ConnectorConnected {
        connected: bool,
    },
    ConnectorType {
        ty: ConnectorType,
    },
    ConnectorMode {
        width: i32,
        height: i32,
        refresh_millihz: u32,
    },
    GetFullscreen {
        fullscreen: bool,
    },
    GetDeviceConnectors {
        connectors: Vec<Connector>,
    },
    GetDrmDeviceSyspath {
        syspath: String,
    },
    GetDrmDeviceVendor {
        vendor: String,
    },
    GetDrmDeviceModel {
        model: String,
    },
    GetDrmDevices {
        devices: Vec<DrmDevice>,
    },
    GetDrmDevicePciId {
        pci_id: PciId,
    },
    GetFloating {
        floating: bool,
    },
    GetColor {
        color: Color,
    },
    GetFont {
        font: String,
    },
}

#[derive(Encode, Decode, Debug)]
pub enum InitMessage {
    V1(V1InitMessage),
}

#[derive(Encode, Decode, Debug)]
pub struct V1InitMessage {}

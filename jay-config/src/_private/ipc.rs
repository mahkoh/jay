use {
    crate::{
        input::{
            acceleration::AccelProfile, capability::Capability, FocusFollowsMouseMode, InputDevice,
            Seat, SwitchEvent,
        },
        keyboard::{mods::Modifiers, syms::KeySym, AppMod, Keymap, ModifiedKeySym},
        logging::LogLevel,
        theme::{colors::Colorable, sized::Resizable, Color},
        timer::Timer,
        video::{
            connector_type::ConnectorType, Connector, DrmDevice, Format, GfxApi, TearingMode,
            Transform, VrrMode,
        },
        Axis, Direction, PciId, Workspace,
        _private::{PollableId, WireMode},
        xwayland::XScalingMode,
    },
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(transparent)]
pub struct ServerFeature(u16);

impl ServerFeature {
    pub const NONE: Self = Self(0);
    pub const MOD_MASK: Self = Self(1);
    pub const MOD_MASK_MODAL: Self = Self(2);
}

#[derive(Serialize, Deserialize, Debug)]
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
        app_mod: AppMod,
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
    Idle,
    DevicesEnumerated,
    InterestReady {
        id: PollableId,
        writable: bool,
        res: Result<(), String>,
    },
    Features {
        features: Vec<ServerFeature>,
    },
    InvokeShortcut2 {
        seat: Seat,
        unmasked_mods: Modifiers,
        effective_mods: Modifiers,
        sym: KeySym,
        app_mod: AppMod,
    },
    SwitchEvent {
        seat: Seat,
        input_device: InputDevice,
        event: SwitchEvent,
    },
}

#[derive(Serialize, Deserialize, Debug)]
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
        app_mod: AppMod,
        tunnel: Option<Vec<ModifiedKeySym>>,
    },
    RemoveShortcut {
        seat: Seat,
        mods: Modifiers,
        sym: KeySym,
        app_mod: AppMod,
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
    ConnectorSetScale {
        connector: Connector,
        scale: f64,
    },
    ConnectorGetScale {
        connector: Connector,
    },
    ConnectorSize {
        connector: Connector,
    },
    SetCursorSize {
        seat: Seat,
        size: i32,
    },
    SetTapEnabled {
        device: InputDevice,
        enabled: bool,
    },
    SetDragEnabled {
        device: InputDevice,
        enabled: bool,
    },
    SetDragLockEnabled {
        device: InputDevice,
        enabled: bool,
    },
    SetUseHardwareCursor {
        seat: Seat,
        use_hardware_cursor: bool,
    },
    DisablePointerConstraint {
        seat: Seat,
    },
    ConnectorSetEnabled {
        connector: Connector,
        enabled: bool,
    },
    MakeRenderDevice {
        device: DrmDevice,
    },
    GetSeatWorkspace {
        seat: Seat,
    },
    SetDefaultWorkspaceCapture {
        capture: bool,
    },
    GetDefaultWorkspaceCapture,
    SetWorkspaceCapture {
        workspace: Workspace,
        capture: bool,
    },
    GetWorkspaceCapture {
        workspace: Workspace,
    },
    SetNaturalScrollingEnabled {
        device: InputDevice,
        enabled: bool,
    },
    SetGfxApi {
        device: Option<DrmDevice>,
        api: GfxApi,
    },
    SetDirectScanoutEnabled {
        device: Option<DrmDevice>,
        enabled: bool,
    },
    ConnectorSetTransform {
        connector: Connector,
        transform: Transform,
    },
    SetDoubleClickIntervalUsec {
        usec: u64,
    },
    SetDoubleClickDistance {
        dist: i32,
    },
    ConnectorModes {
        connector: Connector,
    },
    ConnectorSetMode {
        connector: Connector,
        mode: WireMode,
    },
    AddPollable {
        fd: i32,
    },
    RemovePollable {
        id: PollableId,
    },
    AddInterest {
        pollable: PollableId,
        writable: bool,
    },
    Run2 {
        prog: &'a str,
        args: Vec<String>,
        env: Vec<(String, String)>,
        fds: Vec<(i32, i32)>,
    },
    DisableDefaultSeat,
    DestroyKeymap {
        keymap: Keymap,
    },
    GetConnectorName {
        connector: Connector,
    },
    GetConnectorModel {
        connector: Connector,
    },
    GetConnectorManufacturer {
        connector: Connector,
    },
    GetConnectorSerialNumber {
        connector: Connector,
    },
    GetConnectors {
        device: Option<DrmDevice>,
        connected_only: bool,
    },
    ConnectorGetPosition {
        connector: Connector,
    },
    GetConfigDir,
    GetWorkspaces,
    UnsetEnv {
        key: &'a str,
    },
    SetLogLevel {
        level: LogLevel,
    },
    GetDrmDeviceDevnode {
        device: DrmDevice,
    },
    GetInputDeviceSyspath {
        device: InputDevice,
    },
    GetInputDeviceDevnode {
        device: InputDevice,
    },
    SetIdle {
        timeout: Duration,
    },
    MoveToOutput {
        workspace: WorkspaceSource,
        connector: Connector,
    },
    SetExplicitSyncEnabled {
        enabled: bool,
    },
    GetSocketPath,
    DeviceSetKeymap {
        device: InputDevice,
        keymap: Keymap,
    },
    SetForward {
        seat: Seat,
        forward: bool,
    },
    AddShortcut2 {
        seat: Seat,
        mods: Modifiers,
        mod_mask: Modifiers,
        sym: KeySym,
        app_mod: AppMod,
        tunnel: Option<Vec<ModifiedKeySym>>,
    },
    SetFocusFollowsMouseMode {
        seat: Seat,
        mode: FocusFollowsMouseMode,
    },
    SetInputDeviceConnector {
        input_device: InputDevice,
        connector: Connector,
    },
    RemoveInputMapping {
        input_device: InputDevice,
    },
    SetWindowManagementEnabled {
        seat: Seat,
        enabled: bool,
    },
    SetVrrMode {
        connector: Option<Connector>,
        mode: VrrMode,
    },
    SetVrrCursorHz {
        connector: Option<Connector>,
        hz: f64,
    },
    SetTearingMode {
        connector: Option<Connector>,
        mode: TearingMode,
    },
    SetCalibrationMatrix {
        device: InputDevice,
        matrix: [[f32; 3]; 2],
    },
    SetEiSocketEnabled {
        enabled: bool,
    },
    ConnectorSetFormat {
        connector: Connector,
        format: Format,
    },
    SetFlipMargin {
        device: DrmDevice,
        margin: Duration,
    },
    SetUiDragEnabled {
        enabled: bool,
    },
    SetUiDragThreshold {
        threshold: i32,
    },
    SetXScalingMode {
        mode: XScalingMode,
    },
    SetAppMod {
        seat: Seat,
        app_mod: AppMod,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WorkspaceSource {
    Seat(Seat),
    Explicit(Workspace),
}

#[derive(Serialize, Deserialize, Debug)]
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
    GetConnectors {
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
    ConnectorGetScale {
        scale: f64,
    },
    ConnectorSize {
        width: i32,
        height: i32,
    },
    GetSeatWorkspace {
        workspace: Workspace,
    },
    GetDefaultWorkspaceCapture {
        capture: bool,
    },
    GetWorkspaceCapture {
        capture: bool,
    },
    ConnectorModes {
        modes: Vec<WireMode>,
    },
    AddPollable {
        id: Result<PollableId, String>,
    },
    GetConnectorName {
        name: String,
    },
    GetConnectorModel {
        model: String,
    },
    GetConnectorManufacturer {
        manufacturer: String,
    },
    GetConnectorSerialNumber {
        serial_number: String,
    },
    ConnectorGetPosition {
        x: i32,
        y: i32,
    },
    GetConfigDir {
        dir: String,
    },
    GetWorkspaces {
        workspaces: Vec<Workspace>,
    },
    GetDrmDeviceDevnode {
        devnode: String,
    },
    GetInputDeviceSyspath {
        syspath: String,
    },
    GetInputDeviceDevnode {
        devnode: String,
    },
    GetSocketPath {
        path: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum InitMessage {
    V1(V1InitMessage),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct V1InitMessage {}

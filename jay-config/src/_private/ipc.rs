use {
    crate::{
        _private::{ClientCriterionIpc, PollableId, WindowCriterionIpc, WireMode},
        Axis, Direction, PciId, Workspace,
        client::{Client, ClientCapabilities, ClientMatcher},
        input::{
            FocusFollowsMouseMode, InputDevice, LayerDirection, Seat, SwitchEvent, Timeline,
            acceleration::AccelProfile, capability::Capability, clickmethod::ClickMethod,
        },
        keyboard::{Group, Keymap, mods::Modifiers, syms::KeySym},
        logging::LogLevel,
        theme::{BarPosition, Color, colors::Colorable, sized::Resizable},
        timer::Timer,
        video::{
            BlendSpace, ColorSpace, Connector, DrmDevice, Eotf, Format, GfxApi, TearingMode,
            Transform, VrrMode, connector_type::ConnectorType,
        },
        window::{ContentType, TileState, Window, WindowMatcher, WindowType},
        workspace::WorkspaceDisplayOrder,
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
    pub const SHOW_WORKSPACE_ON: Self = Self(2);
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
    },
    SwitchEvent {
        seat: Seat,
        input_device: InputDevice,
        event: SwitchEvent,
    },
    ClientMatcherMatched {
        matcher: ClientMatcher,
        client: Client,
    },
    ClientMatcherUnmatched {
        matcher: ClientMatcher,
        client: Client,
    },
    WindowMatcherMatched {
        matcher: WindowMatcher,
        window: Window,
    },
    WindowMatcherUnmatched {
        matcher: WindowMatcher,
        window: Window,
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
    GetSeatSplit {
        seat: Seat,
    },
    SetStatus {
        status: &'a str,
    },
    SetSeatSplit {
        seat: Seat,
        axis: Axis,
    },
    GetSeatMono {
        seat: Seat,
    },
    SetSeatMono {
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
    SeatFocus {
        seat: Seat,
        direction: Direction,
    },
    SeatMove {
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
    CreateSeatSplit {
        seat: Seat,
        axis: Axis,
    },
    SeatClose {
        seat: Seat,
    },
    FocusSeatParent {
        seat: Seat,
    },
    GetSeatFloating {
        seat: Seat,
    },
    SetSeatFloating {
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
    SetSeatWorkspace {
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
    SetSeatFullscreen {
        seat: Seat,
        fullscreen: bool,
    },
    GetSeatFullscreen {
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
    GetSeatCursorWorkspace {
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
    SetIdleGracePeriod {
        period: Duration,
    },
    SetColorManagementEnabled {
        enabled: bool,
    },
    ConnectorSetColors {
        connector: Connector,
        color_space: ColorSpace,
        eotf: Eotf,
    },
    ConnectorSetBrightness {
        connector: Connector,
        brightness: Option<f64>,
    },
    SetFloatAboveFullscreen {
        above: bool,
    },
    GetFloatAboveFullscreen,
    GetSeatFloatPinned {
        seat: Seat,
    },
    SetSeatFloatPinned {
        seat: Seat,
        pinned: bool,
    },
    SetShowFloatPinIcon {
        show: bool,
    },
    GetSeatKeyboardWorkspace {
        seat: Seat,
    },
    GetConnectorActiveWorkspace {
        connector: Connector,
    },
    GetConnectorWorkspaces {
        connector: Connector,
    },
    GetClients,
    ClientExists {
        client: Client,
    },
    ClientKill {
        client: Client,
    },
    ClientIsXwayland {
        client: Client,
    },
    WindowExists {
        window: Window,
    },
    GetWindowClient {
        window: Window,
    },
    GetWorkspaceWindow {
        workspace: Workspace,
    },
    GetSeatKeyboardWindow {
        seat: Seat,
    },
    SeatFocusWindow {
        seat: Seat,
        window: Window,
    },
    GetWindowTitle {
        window: Window,
    },
    GetWindowType {
        window: Window,
    },
    GetWindowId {
        window: Window,
    },
    GetWindowIsVisible {
        window: Window,
    },
    GetWindowParent {
        window: Window,
    },
    GetWindowWorkspace {
        window: Window,
    },
    GetWindowChildren {
        window: Window,
    },
    GetWindowSplit {
        window: Window,
    },
    SetWindowSplit {
        window: Window,
        axis: Axis,
    },
    GetWindowMono {
        window: Window,
    },
    SetWindowMono {
        window: Window,
        mono: bool,
    },
    WindowMove {
        window: Window,
        direction: Direction,
    },
    CreateWindowSplit {
        window: Window,
        axis: Axis,
    },
    WindowClose {
        window: Window,
    },
    GetWindowFloating {
        window: Window,
    },
    SetWindowFloating {
        window: Window,
        floating: bool,
    },
    SetWindowWorkspace {
        window: Window,
        workspace: Workspace,
    },
    SetWindowFullscreen {
        window: Window,
        fullscreen: bool,
    },
    GetWindowFullscreen {
        window: Window,
    },
    GetWindowFloatPinned {
        window: Window,
    },
    SetWindowFloatPinned {
        window: Window,
        pinned: bool,
    },
    CreateClientMatcher {
        criterion: ClientCriterionIpc,
    },
    DestroyClientMatcher {
        matcher: ClientMatcher,
    },
    EnableClientMatcherEvents {
        matcher: ClientMatcher,
    },
    CreateWindowMatcher {
        criterion: WindowCriterionIpc,
    },
    DestroyWindowMatcher {
        matcher: WindowMatcher,
    },
    EnableWindowMatcherEvents {
        matcher: WindowMatcher,
    },
    SetWindowMatcherAutoFocus {
        matcher: WindowMatcher,
        auto_focus: bool,
    },
    SetWindowMatcherInitialTileState {
        matcher: WindowMatcher,
        tile_state: TileState,
    },
    SetPointerRevertKey {
        seat: Seat,
        key: KeySym,
    },
    SetClickMethod {
        device: InputDevice,
        method: ClickMethod,
    },
    SetMiddleButtonEmulationEnabled {
        device: InputDevice,
        enabled: bool,
    },
    GetContentType {
        window: Window,
    },
    SetShowBar {
        show: bool,
    },
    GetShowBar,
    SeatFocusHistory {
        seat: Seat,
        timeline: Timeline,
    },
    SeatFocusHistorySetOnlyVisible {
        seat: Seat,
        only_visible: bool,
    },
    SeatFocusHistorySetSameWorkspace {
        seat: Seat,
        same_workspace: bool,
    },
    SeatFocusLayerRel {
        seat: Seat,
        direction: LayerDirection,
    },
    SeatFocusTiles {
        seat: Seat,
    },
    SetMiddleClickPasteEnabled {
        enabled: bool,
    },
    SeatCreateMark {
        seat: Seat,
        kc: Option<u32>,
    },
    SeatJumpToMark {
        seat: Seat,
        kc: Option<u32>,
    },
    SeatCopyMark {
        seat: Seat,
        src: u32,
        dst: u32,
    },
    SetWorkspaceDisplayOrder {
        order: WorkspaceDisplayOrder,
    },
    ConnectorSetBlendSpace {
        connector: Connector,
        blend_space: BlendSpace,
    },
    SetBarFont {
        font: &'a str,
    },
    SetTitleFont {
        font: &'a str,
    },
    SetClientMatcherCapabilities {
        matcher: ClientMatcher,
        caps: ClientCapabilities,
    },
    SetClientMatcherBoundingCapabilities {
        matcher: ClientMatcher,
        caps: ClientCapabilities,
    },
    ShowWorkspaceOn {
        seat: Seat,
        workspace: Workspace,
        connector: Connector,
    },
    SeatSetSimpleImEnabled {
        seat: Seat,
        enabled: bool,
    },
    SeatGetSimpleImEnabled {
        seat: Seat,
    },
    SeatReloadSimpleIm {
        seat: Seat,
    },
    SeatEnableUnicodeInput {
        seat: Seat,
    },
    SetShowTitles {
        show: bool,
    },
    GetShowTitles,
    GetWorkspaceConnector {
        workspace: Workspace,
    },
    GetConnectorInDirection {
        connector: Connector,
        direction: Direction,
    },
    SetBarPosition {
        position: BarPosition,
    },
    GetBarPosition,
    ConnectorSetUseNativeGamut {
        connector: Connector,
        use_native_gamut: bool,
    },
    KeymapFromNames {
        rules: Option<&'a str>,
        model: Option<&'a str>,
        groups: Option<Vec<Group<'a>>>,
        options: Option<Vec<&'a str>>,
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
    GetSeatCursorWorkspace {
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
    GetFloatAboveFullscreen {
        above: bool,
    },
    GetFloatPinned {
        pinned: bool,
    },
    GetSeatKeyboardWorkspace {
        workspace: Workspace,
    },
    GetConnectorActiveWorkspace {
        workspace: Workspace,
    },
    GetConnectorWorkspaces {
        workspaces: Vec<Workspace>,
    },
    GetClients {
        clients: Vec<Client>,
    },
    ClientExists {
        exists: bool,
    },
    ClientIsXwayland {
        is_xwayland: bool,
    },
    WindowExists {
        exists: bool,
    },
    GetWindowClient {
        client: Client,
    },
    GetSeatKeyboardWindow {
        window: Window,
    },
    GetWorkspaceWindow {
        window: Window,
    },
    GetWindowParent {
        window: Window,
    },
    GetWindowChildren {
        windows: Vec<Window>,
    },
    GetWindowTitle {
        title: String,
    },
    GetWindowType {
        kind: WindowType,
    },
    GetWindowId {
        id: String,
    },
    GetWindowWorkspace {
        workspace: Workspace,
    },
    GetWindowFloating {
        floating: bool,
    },
    GetWindowSplit {
        axis: Axis,
    },
    GetWindowMono {
        mono: bool,
    },
    GetWindowFullscreen {
        fullscreen: bool,
    },
    GetWindowFloatPinned {
        pinned: bool,
    },
    GetWindowIsVisible {
        visible: bool,
    },
    CreateClientMatcher {
        matcher: ClientMatcher,
    },
    CreateWindowMatcher {
        matcher: WindowMatcher,
    },
    GetContentType {
        kind: ContentType,
    },
    GetShowBar {
        show: bool,
    },
    SeatGetSimpleImEnabled {
        enabled: bool,
    },
    GetShowTitles {
        show: bool,
    },
    GetWorkspaceConnector {
        connector: Connector,
    },
    GetConnectorInDirection {
        connector: Connector,
    },
    GetBarPosition {
        position: BarPosition,
    },
    KeymapFromNames {
        keymap: Keymap,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum InitMessage {
    V1(V1InitMessage),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct V1InitMessage {}

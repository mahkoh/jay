#![allow(clippy::declare_interior_mutable_const, clippy::type_complexity)]

use {
    crate::{
        _private::{
            ClientCriterionIpc, ClientCriterionStringField, Config, ConfigEntry, ConfigEntryGen,
            GenericCriterionIpc, PollableId, VERSION, WindowCriterionIpc,
            WindowCriterionStringField, WireMode, bincode_ops,
            ipc::{
                ClientMessage, InitMessage, Response, ServerFeature, ServerMessage, WorkspaceSource,
            },
            logging,
        },
        Axis, Direction, ModifiedKeySym, PciId, Workspace,
        client::{Client, ClientCapabilities, ClientCriterion, ClientMatcher, MatchedClient},
        exec::Command,
        input::{
            FocusFollowsMouseMode, InputDevice, LayerDirection, Seat, SwitchEvent, Timeline,
            acceleration::AccelProfile, capability::Capability, clickmethod::ClickMethod,
        },
        keyboard::{
            Keymap,
            mods::{Modifiers, RELEASE},
            syms::KeySym,
        },
        logging::LogLevel,
        tasks::{JoinHandle, JoinSlot},
        theme::{Color, colors::Colorable, sized::Resizable},
        timer::Timer,
        video::{
            BlendSpace, ColorSpace, Connector, DrmDevice, Eotf, Format, GfxApi, Mode, TearingMode,
            Transform, VrrMode,
            connector_type::{CON_UNKNOWN, ConnectorType},
        },
        window::{
            ContentType, MatchedWindow, TileState, Window, WindowCriterion, WindowMatcher,
            WindowType,
        },
        workspace::WorkspaceDisplayOrder,
        xwayland::XScalingMode,
    },
    bincode::Options,
    futures_util::task::ArcWake,
    run_on_drop::{OnDrop, on_drop},
    std::{
        cell::{Cell, RefCell},
        collections::{HashMap, VecDeque, hash_map::Entry},
        future::Future,
        mem,
        ops::Deref,
        os::fd::IntoRawFd,
        panic::{AssertUnwindSafe, catch_unwind},
        pin::Pin,
        ptr,
        rc::Rc,
        slice,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering::Relaxed},
        },
        task::{Context, Poll, Waker},
        time::Duration,
    },
};

type Callback<T = ()> = Rc<RefCell<dyn FnMut(T)>>;

fn cb<T, F: FnMut(T) + 'static>(f: F) -> Callback<T> {
    Rc::new(RefCell::new(f))
}

fn run_cb<T>(name: &str, cb: &Callback<T>, t: T) {
    match cb.try_borrow_mut() {
        Ok(mut cb) => ignore_panic(name, || cb(t)),
        Err(_) => log::error!("Cannot invoke {name} callback because it is already running"),
    }
}

fn ignore_panic(name: &str, f: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        log::error!("A panic occurred in a {name} callback");
    }
}

struct KeyHandler {
    registered_mask: Modifiers,
    cb_mask: Modifiers,
    cb: Option<Callback>,
    latched: Vec<Box<dyn FnOnce()>>,
}

pub(crate) struct ConfigClient {
    configure: extern "C" fn(),
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    key_handlers: RefCell<HashMap<(Seat, ModifiedKeySym), KeyHandler>>,
    timer_handlers: RefCell<HashMap<Timer, Callback>>,
    response: RefCell<Vec<Response>>,
    on_new_seat: RefCell<Option<Callback<Seat>>>,
    on_new_input_device: RefCell<Option<Callback<InputDevice>>>,
    on_input_device_removed: RefCell<Option<Callback<InputDevice>>>,
    on_connector_connected: RefCell<Option<Callback<Connector>>>,
    on_connector_disconnected: RefCell<Option<Callback<Connector>>>,
    on_graphics_initialized: Cell<Option<Box<dyn FnOnce()>>>,
    on_devices_enumerated: Cell<Option<Box<dyn FnOnce()>>>,
    on_new_connector: RefCell<Option<Callback<Connector>>>,
    on_new_drm_device: RefCell<Option<Callback<DrmDevice>>>,
    on_del_drm_device: RefCell<Option<Callback<DrmDevice>>>,
    on_idle: RefCell<Option<Callback>>,
    on_switch_event: RefCell<HashMap<InputDevice, Callback<SwitchEvent>>>,
    on_unload: Cell<Option<OnDrop<Box<dyn FnOnce()>>>>,
    bufs: RefCell<Vec<Vec<u8>>>,
    reload: Cell<bool>,
    read_interests: RefCell<HashMap<PollableId, Interest>>,
    write_interests: RefCell<HashMap<PollableId, Interest>>,
    tasks: Tasks,
    status_task: Cell<Vec<JoinHandle<()>>>,
    i3bar_separator: RefCell<Option<Rc<String>>>,
    pressed_keysym: Cell<Option<KeySym>>,
    client_match_handlers: RefCell<HashMap<ClientMatcher, ClientMatchHandler>>,
    window_match_handlers: RefCell<HashMap<WindowMatcher, WindowMatchHandler>>,

    feat_mod_mask: Cell<bool>,
}

struct ClientMatchHandler {
    cb: Callback<MatchedClient>,
    latched: HashMap<Client, Box<dyn FnOnce()>>,
}

struct WindowMatchHandler {
    cb: Callback<MatchedWindow>,
    latched: HashMap<Window, Box<dyn FnOnce()>>,
}

struct Interest {
    result: Option<Result<(), String>>,
    waker: Option<Waker>,
}

#[derive(Default)]
struct Tasks {
    last_id: Cell<u64>,
    ready_front: Cell<VecDeque<u64>>,
    ready_back: Arc<TasksBackBuffer>,
    tasks: RefCell<HashMap<u64, Rc<RefCell<Task>>>>,
}

#[derive(Default)]
struct TasksBackBuffer {
    any: AtomicBool,
    tasks: Mutex<VecDeque<u64>>,
}

impl TasksBackBuffer {
    fn append(&self, task: u64) {
        self.tasks.lock().unwrap().push_back(task);
        self.any.store(true, Relaxed);
    }
}

struct Task {
    task: Pin<Box<dyn Future<Output = ()>>>,
    waker: Waker,
}

impl Drop for ConfigClient {
    fn drop(&mut self) {
        unsafe {
            (self.srv_unref)(self.srv_data);
        }
    }
}

thread_local! {
    pub(crate) static CLIENT: Cell<*const ConfigClient> = const { Cell::new(ptr::null()) };
}

unsafe fn with_client<T, F: FnOnce(&ConfigClient) -> T>(data: *const u8, f: F) -> T {
    struct Reset<'a> {
        cell: &'a Cell<*const ConfigClient>,
        val: *const ConfigClient,
    }
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.cell.set(self.val);
        }
    }
    CLIENT.with(|cell| unsafe {
        let client = data as *const ConfigClient;
        Rc::increment_strong_count(client);
        let client = Rc::from_raw(client);
        let old = cell.replace(client.deref());
        let _reset = Reset { cell, val: old };
        f(&client)
    })
}

impl<T: Config> ConfigEntryGen<T> {
    pub const ENTRY: ConfigEntry = ConfigEntry {
        version: VERSION,
        init: Self::init,
        unref,
        handle_msg,
    };

    pub unsafe extern "C" fn init(
        srv_data: *const u8,
        srv_unref: unsafe extern "C" fn(data: *const u8),
        srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
        init_data: *const u8,
        size: usize,
    ) -> *const u8 {
        logging::init();
        unsafe {
            init(
                srv_data,
                srv_unref,
                srv_handler,
                init_data,
                size,
                T::configure,
            )
        }
    }
}

pub unsafe extern "C" fn init(
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    init: *const u8,
    size: usize,
    f: extern "C" fn(),
) -> *const u8 {
    let client = Rc::new(ConfigClient {
        configure: f,
        srv_data,
        srv_unref,
        srv_handler,
        key_handlers: Default::default(),
        timer_handlers: Default::default(),
        response: Default::default(),
        on_new_seat: Default::default(),
        on_new_input_device: Default::default(),
        on_input_device_removed: Default::default(),
        on_connector_connected: Default::default(),
        on_connector_disconnected: Default::default(),
        on_graphics_initialized: Default::default(),
        on_devices_enumerated: Default::default(),
        on_new_connector: Default::default(),
        on_new_drm_device: Default::default(),
        on_del_drm_device: Default::default(),
        on_idle: Default::default(),
        on_switch_event: Default::default(),
        on_unload: Default::default(),
        bufs: Default::default(),
        reload: Cell::new(false),
        read_interests: Default::default(),
        write_interests: Default::default(),
        tasks: Default::default(),
        status_task: Default::default(),
        i3bar_separator: Default::default(),
        pressed_keysym: Cell::new(None),
        client_match_handlers: Default::default(),
        window_match_handlers: Default::default(),
        feat_mod_mask: Cell::new(false),
    });
    let init = unsafe { slice::from_raw_parts(init, size) };
    client.handle_init_msg(init);
    Rc::into_raw(client) as *const u8
}

pub unsafe extern "C" fn unref(data: *const u8) {
    let client = data as *const ConfigClient;
    unsafe {
        drop(Rc::from_raw(client));
    }
}

pub unsafe extern "C" fn handle_msg(data: *const u8, msg: *const u8, size: usize) {
    unsafe {
        with_client(data, |client| {
            let msg = slice::from_raw_parts(msg, size);
            client.handle_msg(msg);
        });
    }
}

macro_rules! get_response {
    ($res:expr, $def:expr, $ty:ident { $($field:ident),+ }) => {
        let ($($field,)+) = match $res {
            Response::$ty { $($field,)+ } => ($($field,)+),
            _ => {
                log::error!("Server did not send a response to a {} request", stringify!($ty));
                return $def;
            }
        };
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
enum GenericCriterion<'a, Crit, Matcher> {
    Matcher(Matcher),
    Not(&'a Crit),
    All(&'a [Crit]),
    Any(&'a [Crit]),
    Exactly(usize, &'a [Crit]),
}

impl ConfigClient {
    fn send(&self, msg: &ClientMessage) {
        let mut buf = self.bufs.borrow_mut().pop().unwrap_or_default();
        buf.clear();
        bincode_ops().serialize_into(&mut buf, msg).unwrap();
        unsafe {
            (self.srv_handler)(self.srv_data, buf.as_ptr(), buf.len());
        }
        self.bufs.borrow_mut().push(buf);
    }

    fn send_with_response(&self, msg: &ClientMessage) -> Response {
        self.with_response(|| self.send(msg))
    }

    pub fn reload(&self) {
        self.send(&ClientMessage::Reload);
    }

    pub fn is_reload(&self) -> bool {
        self.reload.get()
    }

    pub fn spawn(&self, command: &Command) {
        let env = command
            .env
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let fds: Vec<_> = command
            .fds
            .borrow_mut()
            .drain()
            .map(|(a, b)| (a, b.into_raw_fd()))
            .collect();
        if fds.is_empty() {
            self.send(&ClientMessage::Run {
                prog: &command.prog,
                args: command.args.clone(),
                env,
            });
        } else {
            self.send(&ClientMessage::Run2 {
                prog: &command.prog,
                args: command.args.clone(),
                env,
                fds,
            });
        }
    }

    pub fn grab(&self, kb: InputDevice, grab: bool) {
        self.send(&ClientMessage::GrabKb { kb, grab });
    }

    pub fn seat_focus_history(&self, seat: Seat, timeline: Timeline) {
        self.send(&ClientMessage::SeatFocusHistory { seat, timeline });
    }

    pub fn seat_focus_history_set_only_visible(&self, seat: Seat, only_visible: bool) {
        self.send(&ClientMessage::SeatFocusHistorySetOnlyVisible { seat, only_visible });
    }

    pub fn seat_focus_history_set_same_workspace(&self, seat: Seat, same_workspace: bool) {
        self.send(&ClientMessage::SeatFocusHistorySetSameWorkspace {
            seat,
            same_workspace,
        });
    }

    pub fn seat_focus_layer_rel(&self, seat: Seat, direction: LayerDirection) {
        self.send(&ClientMessage::SeatFocusLayerRel { seat, direction });
    }

    pub fn seat_focus_tiles(&self, seat: Seat) {
        self.send(&ClientMessage::SeatFocusTiles { seat });
    }

    pub fn seat_focus(&self, seat: Seat, direction: Direction) {
        self.send(&ClientMessage::SeatFocus { seat, direction });
    }

    pub fn seat_move(&self, seat: Seat, direction: Direction) {
        self.send(&ClientMessage::SeatMove { seat, direction });
    }

    pub fn window_move(&self, window: Window, direction: Direction) {
        self.send(&ClientMessage::WindowMove { window, direction });
    }

    pub fn window_exists(&self, window: Window) -> bool {
        let res = self.send_with_response(&ClientMessage::WindowExists { window });
        get_response!(res, false, WindowExists { exists });
        exists
    }

    pub fn window_client(&self, window: Window) -> Client {
        let res = self.send_with_response(&ClientMessage::GetWindowClient { window });
        get_response!(res, Client(0), GetWindowClient { client });
        client
    }

    pub fn get_workspace_window(&self, workspace: Workspace) -> Window {
        let res = self.send_with_response(&ClientMessage::GetWorkspaceWindow { workspace });
        get_response!(res, Window(0), GetWorkspaceWindow { window });
        window
    }

    pub fn get_seat_keyboard_window(&self, seat: Seat) -> Window {
        let res = self.send_with_response(&ClientMessage::GetSeatKeyboardWindow { seat });
        get_response!(res, Window(0), GetSeatKeyboardWindow { window });
        window
    }

    pub fn focus_window(&self, seat: Seat, window: Window) {
        self.send(&ClientMessage::SeatFocusWindow { seat, window });
    }

    pub fn window_title(&self, window: Window) -> String {
        let res = self.send_with_response(&ClientMessage::GetWindowTitle { window });
        get_response!(res, String::new(), GetWindowTitle { title });
        title
    }

    pub fn window_type(&self, window: Window) -> WindowType {
        let res = self.send_with_response(&ClientMessage::GetWindowType { window });
        get_response!(res, WindowType(0), GetWindowType { kind });
        kind
    }

    pub fn content_type(&self, window: Window) -> ContentType {
        let res = self.send_with_response(&ClientMessage::GetContentType { window });
        get_response!(res, ContentType(0), GetContentType { kind });
        kind
    }

    pub fn window_id(&self, window: Window) -> String {
        let res = self.send_with_response(&ClientMessage::GetWindowId { window });
        get_response!(res, String::new(), GetWindowId { id });
        id
    }

    pub fn window_parent(&self, window: Window) -> Window {
        let res = self.send_with_response(&ClientMessage::GetWindowParent { window });
        get_response!(res, Window(0), GetWindowParent { window });
        window
    }

    pub fn window_children(&self, window: Window) -> Vec<Window> {
        let res = self.send_with_response(&ClientMessage::GetWindowChildren { window });
        get_response!(res, vec![], GetWindowChildren { windows });
        windows
    }

    pub fn window_is_visible(&self, window: Window) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWindowIsVisible { window });
        get_response!(res, false, GetWindowIsVisible { visible });
        visible
    }

    pub fn unbind<T: Into<ModifiedKeySym>>(&self, seat: Seat, mod_sym: T) {
        let mod_sym = mod_sym.into();
        if let Entry::Occupied(mut oe) = self.key_handlers.borrow_mut().entry((seat, mod_sym)) {
            oe.get_mut().cb = None;
            if oe.get().latched.is_empty() {
                oe.remove();
                self.send(&ClientMessage::RemoveShortcut {
                    seat,
                    mods: mod_sym.mods,
                    sym: mod_sym.sym,
                })
            }
        }
    }

    fn with_response<F: FnOnce()>(&self, f: F) -> Response {
        f();
        self.response.borrow_mut().pop().unwrap_or(Response::None)
    }

    pub fn seats(&self) -> Vec<Seat> {
        let res = self.send_with_response(&ClientMessage::GetSeats);
        get_response!(res, vec![], GetSeats { seats });
        seats
    }

    pub fn seat_mono(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetSeatMono { seat });
        get_response!(res, false, GetMono { mono });
        mono
    }

    pub fn window_mono(&self, window: Window) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWindowMono { window });
        get_response!(res, false, GetWindowMono { mono });
        mono
    }

    pub fn get_timer(&self, name: &str) -> Timer {
        let res = self.send_with_response(&ClientMessage::GetTimer { name });
        get_response!(res, Timer(0), GetTimer { timer });
        timer
    }

    pub fn remove_timer(&self, timer: Timer) {
        self.send(&ClientMessage::RemoveTimer { timer });
    }

    pub fn program_timer(
        &self,
        timer: Timer,
        initial: Option<Duration>,
        periodic: Option<Duration>,
    ) {
        self.send(&ClientMessage::ProgramTimer {
            timer,
            initial,
            periodic,
        });
    }

    pub fn on_timer_tick<F: FnMut() + 'static>(&self, timer: Timer, mut f: F) {
        self.timer_handlers
            .borrow_mut()
            .insert(timer, cb(move |_| f()));
    }

    pub fn get_workspace(&self, name: &str) -> Workspace {
        let res = self.send_with_response(&ClientMessage::GetWorkspace { name });
        get_response!(res, Workspace(0), GetWorkspace { workspace });
        workspace
    }

    pub fn get_connector(&self, ty: ConnectorType, idx: u32) -> Connector {
        let res = self.send_with_response(&ClientMessage::GetConnector { ty, idx });
        get_response!(res, Connector(0), GetConnector { connector });
        connector
    }

    pub fn get_seat_workspace(&self, seat: Seat) -> Workspace {
        let res = self.send_with_response(&ClientMessage::GetSeatWorkspace { seat });
        get_response!(res, Workspace(0), GetSeatWorkspace { workspace });
        workspace
    }

    pub fn get_window_workspace(&self, window: Window) -> Workspace {
        let res = self.send_with_response(&ClientMessage::GetWindowWorkspace { window });
        get_response!(res, Workspace(0), GetWindowWorkspace { workspace });
        workspace
    }

    pub fn get_seat_keyboard_workspace(&self, seat: Seat) -> Workspace {
        let res = self.send_with_response(&ClientMessage::GetSeatKeyboardWorkspace { seat });
        get_response!(res, Workspace(0), GetSeatKeyboardWorkspace { workspace });
        workspace
    }

    pub fn set_default_workspace_capture(&self, capture: bool) {
        self.send(&ClientMessage::SetDefaultWorkspaceCapture { capture });
    }

    pub fn set_workspace_capture(&self, workspace: Workspace, capture: bool) {
        self.send(&ClientMessage::SetWorkspaceCapture { workspace, capture });
    }

    pub fn get_default_workspace_capture(&self) -> bool {
        let res = self.send_with_response(&ClientMessage::GetDefaultWorkspaceCapture);
        get_response!(res, true, GetDefaultWorkspaceCapture { capture });
        capture
    }

    pub fn get_workspace_capture(&self, workspace: Workspace) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWorkspaceCapture { workspace });
        get_response!(res, true, GetWorkspaceCapture { capture });
        capture
    }

    pub fn show_workspace(&self, seat: Seat, workspace: Workspace) {
        self.send(&ClientMessage::ShowWorkspace { seat, workspace });
    }

    pub fn set_seat_workspace(&self, seat: Seat, workspace: Workspace) {
        self.send(&ClientMessage::SetSeatWorkspace { seat, workspace });
    }

    pub fn set_window_workspace(&self, window: Window, workspace: Workspace) {
        self.send(&ClientMessage::SetWindowWorkspace { window, workspace });
    }

    pub fn seat_split(&self, seat: Seat) -> Axis {
        let res = self.send_with_response(&ClientMessage::GetSeatSplit { seat });
        get_response!(res, Axis::Horizontal, GetSplit { axis });
        axis
    }

    pub fn window_split(&self, window: Window) -> Axis {
        let res = self.send_with_response(&ClientMessage::GetWindowSplit { window });
        get_response!(res, Axis::Horizontal, GetWindowSplit { axis });
        axis
    }

    pub fn disable_pointer_constraint(&self, seat: Seat) {
        self.send(&ClientMessage::DisablePointerConstraint { seat });
    }

    pub fn move_to_output(&self, workspace: WorkspaceSource, connector: Connector) {
        self.send(&ClientMessage::MoveToOutput {
            workspace,
            connector,
        });
    }

    pub fn set_seat_fullscreen(&self, seat: Seat, fullscreen: bool) {
        self.send(&ClientMessage::SetSeatFullscreen { seat, fullscreen });
    }

    pub fn get_seat_fullscreen(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetSeatFullscreen { seat });
        get_response!(res, false, GetFullscreen { fullscreen });
        fullscreen
    }

    pub fn set_window_fullscreen(&self, window: Window, fullscreen: bool) {
        self.send(&ClientMessage::SetWindowFullscreen { window, fullscreen });
    }

    pub fn get_window_fullscreen(&self, window: Window) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWindowFullscreen { window });
        get_response!(res, false, GetWindowFullscreen { fullscreen });
        fullscreen
    }

    pub fn reset_font(&self) {
        self.send(&ClientMessage::ResetFont);
    }

    pub fn set_font(&self, font: &str) {
        self.send(&ClientMessage::SetFont { font });
    }

    pub fn set_bar_font(&self, font: &str) {
        self.send(&ClientMessage::SetBarFont { font });
    }

    pub fn set_title_font(&self, font: &str) {
        self.send(&ClientMessage::SetTitleFont { font });
    }

    pub fn get_font(&self) -> String {
        let res = self.send_with_response(&ClientMessage::GetFont);
        get_response!(res, String::new(), GetFont { font });
        font
    }

    pub fn get_seat_floating(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetSeatFloating { seat });
        get_response!(res, false, GetFloating { floating });
        floating
    }

    pub fn set_seat_floating(&self, seat: Seat, floating: bool) {
        self.send(&ClientMessage::SetSeatFloating { seat, floating });
    }

    pub fn toggle_seat_floating(&self, seat: Seat) {
        self.set_seat_floating(seat, !self.get_seat_floating(seat));
    }

    pub fn get_window_floating(&self, window: Window) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWindowFloating { window });
        get_response!(res, false, GetWindowFloating { floating });
        floating
    }

    pub fn set_window_floating(&self, window: Window, floating: bool) {
        self.send(&ClientMessage::SetWindowFloating { window, floating });
    }

    pub fn reset_colors(&self) {
        self.send(&ClientMessage::ResetColors);
    }

    pub fn reset_sizes(&self) {
        self.send(&ClientMessage::ResetSizes);
    }

    pub fn get_color(&self, colorable: Colorable) -> Color {
        let res = self.send_with_response(&ClientMessage::GetColor { colorable });
        get_response!(res, Color::BLACK, GetColor { color });
        color
    }

    pub fn set_color(&self, colorable: Colorable, color: Color) {
        self.send(&ClientMessage::SetColor { colorable, color });
    }

    pub fn get_size(&self, sized: Resizable) -> i32 {
        let res = self.send_with_response(&ClientMessage::GetSize { sized });
        get_response!(res, 0, GetSize { size });
        size
    }

    pub fn set_cursor_size(&self, seat: Seat, size: i32) {
        self.send(&ClientMessage::SetCursorSize { seat, size })
    }

    pub fn set_use_hardware_cursor(&self, seat: Seat, use_hardware_cursor: bool) {
        self.send(&ClientMessage::SetUseHardwareCursor {
            seat,
            use_hardware_cursor,
        })
    }

    pub fn set_size(&self, sized: Resizable, size: i32) {
        self.send(&ClientMessage::SetSize { sized, size })
    }

    pub fn set_seat_mono(&self, seat: Seat, mono: bool) {
        self.send(&ClientMessage::SetSeatMono { seat, mono });
    }

    pub fn set_window_mono(&self, window: Window, mono: bool) {
        self.send(&ClientMessage::SetWindowMono { window, mono });
    }

    pub fn set_env(&self, key: &str, val: &str) {
        self.send(&ClientMessage::SetEnv { key, val });
    }

    pub fn set_log_level(&self, level: LogLevel) {
        self.send(&ClientMessage::SetLogLevel { level })
    }

    pub fn unset_env(&self, key: &str) {
        self.send(&ClientMessage::UnsetEnv { key });
    }

    pub fn set_status(&self, status: &str) {
        self.send(&ClientMessage::SetStatus { status });
    }

    pub fn set_status_tasks(&self, tasks: Vec<JoinHandle<()>>) {
        for old in self.status_task.replace(tasks) {
            old.abort();
        }
    }

    pub fn set_i3bar_separator(&self, separator: &str) {
        *self.i3bar_separator.borrow_mut() = Some(Rc::new(separator.to_string()));
    }

    pub fn get_i3bar_separator(&self) -> Option<Rc<String>> {
        self.i3bar_separator.borrow().clone()
    }

    pub fn set_seat_split(&self, seat: Seat, axis: Axis) {
        self.send(&ClientMessage::SetSeatSplit { seat, axis });
    }

    pub fn set_window_split(&self, window: Window, axis: Axis) {
        self.send(&ClientMessage::SetWindowSplit { window, axis });
    }

    pub fn create_seat_split(&self, seat: Seat, axis: Axis) {
        self.send(&ClientMessage::CreateSeatSplit { seat, axis });
    }

    pub fn create_window_split(&self, window: Window, axis: Axis) {
        self.send(&ClientMessage::CreateWindowSplit { window, axis });
    }

    pub fn seat_close(&self, seat: Seat) {
        self.send(&ClientMessage::SeatClose { seat });
    }

    pub fn close_window(&self, window: Window) {
        self.send(&ClientMessage::WindowClose { window });
    }

    pub fn focus_seat_parent(&self, seat: Seat) {
        self.send(&ClientMessage::FocusSeatParent { seat });
    }

    pub fn get_seat(&self, name: &str) -> Seat {
        let res = self.send_with_response(&ClientMessage::GetSeat { name });
        get_response!(res, Seat(0), GetSeat { seat });
        seat
    }

    pub fn get_input_devices(&self, seat: Option<Seat>) -> Vec<InputDevice> {
        let res = self.send_with_response(&ClientMessage::GetInputDevices { seat });
        get_response!(res, vec!(), GetInputDevices { devices });
        devices
    }

    pub fn on_new_seat<F: FnMut(Seat) + 'static>(&self, f: F) {
        *self.on_new_seat.borrow_mut() = Some(cb(f));
    }

    pub fn quit(&self) {
        self.send(&ClientMessage::Quit)
    }

    pub fn switch_to_vt(&self, vtnr: u32) {
        self.send(&ClientMessage::SwitchTo { vtnr })
    }

    pub fn on_new_input_device<F: FnMut(InputDevice) + 'static>(&self, f: F) {
        *self.on_new_input_device.borrow_mut() = Some(cb(f));
    }

    pub fn on_input_device_removed<F: FnMut(InputDevice) + 'static>(&self, f: F) {
        *self.on_input_device_removed.borrow_mut() = Some(cb(f));
    }

    pub fn on_switch_event<F: FnMut(SwitchEvent) + 'static>(
        &self,
        input_device: InputDevice,
        f: F,
    ) {
        self.on_switch_event
            .borrow_mut()
            .insert(input_device, cb(f));
    }

    pub fn set_double_click_interval(&self, usec: u64) {
        self.send(&ClientMessage::SetDoubleClickIntervalUsec { usec });
    }

    pub fn set_double_click_distance(&self, dist: i32) {
        self.send(&ClientMessage::SetDoubleClickDistance { dist });
    }

    pub fn disable_default_seat(&self) {
        self.send(&ClientMessage::DisableDefaultSeat);
    }

    pub fn connector_get_position(&self, connector: Connector) -> (i32, i32) {
        let res = self.send_with_response(&ClientMessage::ConnectorGetPosition { connector });
        get_response!(res, (0, 0), ConnectorGetPosition { x, y });
        (x, y)
    }

    pub fn connector_set_position(&self, connector: Connector, x: i32, y: i32) {
        self.send(&ClientMessage::ConnectorSetPosition { connector, x, y });
    }

    pub fn connector_set_enabled(&self, connector: Connector, enabled: bool) {
        self.send(&ClientMessage::ConnectorSetEnabled { connector, enabled });
    }

    pub fn connector_set_transform(&self, connector: Connector, transform: Transform) {
        self.send(&ClientMessage::ConnectorSetTransform {
            connector,
            transform,
        });
    }

    pub fn connector_get_name(&self, connector: Connector) -> String {
        let res = self.send_with_response(&ClientMessage::GetConnectorName { connector });
        get_response!(res, String::new(), GetConnectorName { name });
        name
    }

    pub fn connector_get_model(&self, connector: Connector) -> String {
        let res = self.send_with_response(&ClientMessage::GetConnectorModel { connector });
        get_response!(res, String::new(), GetConnectorModel { model });
        model
    }

    pub fn connector_get_manufacturer(&self, connector: Connector) -> String {
        let res = self.send_with_response(&ClientMessage::GetConnectorManufacturer { connector });
        get_response!(
            res,
            String::new(),
            GetConnectorManufacturer { manufacturer }
        );
        manufacturer
    }

    pub fn connector_get_serial_number(&self, connector: Connector) -> String {
        let res = self.send_with_response(&ClientMessage::GetConnectorSerialNumber { connector });
        get_response!(
            res,
            String::new(),
            GetConnectorSerialNumber { serial_number }
        );
        serial_number
    }

    pub fn connectors(&self, device: Option<DrmDevice>) -> Vec<Connector> {
        if let Some(device) = device {
            let res = self.send_with_response(&ClientMessage::GetDeviceConnectors { device });
            get_response!(res, vec![], GetConnectors { connectors });
            return connectors;
        }
        let res = self.send_with_response(&ClientMessage::GetConnectors {
            device,
            connected_only: false,
        });
        get_response!(res, vec![], GetConnectors { connectors });
        connectors
    }

    pub fn drm_device_syspath(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceSyspath { device });
        get_response!(res, String::new(), GetDrmDeviceSyspath { syspath });
        syspath
    }

    pub fn drm_device_devnode(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceDevnode { device });
        get_response!(res, String::new(), GetDrmDeviceDevnode { devnode });
        devnode
    }

    pub fn drm_device_vendor(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceVendor { device });
        get_response!(res, String::new(), GetDrmDeviceVendor { vendor });
        vendor
    }

    pub fn drm_device_model(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceModel { device });
        get_response!(res, String::new(), GetDrmDeviceModel { model });
        model
    }

    pub fn drm_device_pci_id(&self, device: DrmDevice) -> PciId {
        let res = self.send_with_response(&ClientMessage::GetDrmDevicePciId { device });
        get_response!(res, Default::default(), GetDrmDevicePciId { pci_id });
        pci_id
    }

    pub fn make_render_device(&self, device: DrmDevice) {
        self.send(&ClientMessage::MakeRenderDevice { device });
    }

    pub fn set_gfx_api(&self, device: Option<DrmDevice>, api: GfxApi) {
        self.send(&ClientMessage::SetGfxApi { device, api });
    }

    pub fn set_direct_scanout_enabled(&self, device: Option<DrmDevice>, enabled: bool) {
        self.send(&ClientMessage::SetDirectScanoutEnabled { device, enabled });
    }

    pub fn set_flip_margin(&self, device: DrmDevice, margin: Duration) {
        self.send(&ClientMessage::SetFlipMargin { device, margin });
    }

    pub fn set_ui_drag_enabled(&self, enabled: bool) {
        self.send(&ClientMessage::SetUiDragEnabled { enabled });
    }

    pub fn set_ui_drag_threshold(&self, threshold: i32) {
        self.send(&ClientMessage::SetUiDragThreshold { threshold });
    }

    pub fn set_color_management_enabled(&self, enabled: bool) {
        self.send(&ClientMessage::SetColorManagementEnabled { enabled });
    }

    pub fn set_float_above_fullscreen(&self, above: bool) {
        self.send(&ClientMessage::SetFloatAboveFullscreen { above });
    }

    pub fn get_float_above_fullscreen(&self) -> bool {
        let res = self.send_with_response(&ClientMessage::GetFloatAboveFullscreen);
        get_response!(res, false, GetFloatAboveFullscreen { above });
        above
    }

    pub fn set_show_bar(&self, show: bool) {
        self.send(&ClientMessage::SetShowBar { show });
    }

    pub fn get_show_bar(&self) -> bool {
        let res = self.send_with_response(&ClientMessage::GetShowBar);
        get_response!(res, true, GetShowBar { show });
        show
    }

    pub fn set_middle_click_paste_enabled(&self, enabled: bool) {
        self.send(&ClientMessage::SetMiddleClickPasteEnabled { enabled });
    }

    pub fn set_workspace_display_order(&self, order: WorkspaceDisplayOrder) {
        self.send(&ClientMessage::SetWorkspaceDisplayOrder { order });
    }

    pub fn seat_create_mark(&self, seat: Seat, kc: Option<u32>) {
        self.send(&ClientMessage::SeatCreateMark { seat, kc });
    }

    pub fn seat_jump_to_mark(&self, seat: Seat, kc: Option<u32>) {
        self.send(&ClientMessage::SeatJumpToMark { seat, kc });
    }

    pub fn seat_copy_mark(&self, seat: Seat, src: u32, dst: u32) {
        self.send(&ClientMessage::SeatCopyMark { seat, src, dst });
    }

    pub fn set_show_float_pin_icon(&self, show: bool) {
        self.send(&ClientMessage::SetShowFloatPinIcon { show });
    }

    pub fn get_pinned(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetSeatFloatPinned { seat });
        get_response!(res, false, GetFloatPinned { pinned });
        pinned
    }

    pub fn set_pinned(&self, seat: Seat, pinned: bool) {
        self.send(&ClientMessage::SetSeatFloatPinned { seat, pinned });
    }

    pub fn get_window_pinned(&self, window: Window) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWindowFloatPinned { window });
        get_response!(res, false, GetWindowFloatPinned { pinned });
        pinned
    }

    pub fn set_window_pinned(&self, window: Window, pinned: bool) {
        self.send(&ClientMessage::SetWindowFloatPinned { window, pinned });
    }

    pub fn connector_connected(&self, connector: Connector) -> bool {
        let res = self.send_with_response(&ClientMessage::ConnectorConnected { connector });
        get_response!(res, false, ConnectorConnected { connected });
        connected
    }

    pub fn connector_set_scale(&self, connector: Connector, scale: f64) {
        self.send(&ClientMessage::ConnectorSetScale { connector, scale });
    }

    pub fn connector_set_format(&self, connector: Connector, format: Format) {
        self.send(&ClientMessage::ConnectorSetFormat { connector, format });
    }

    pub fn connector_set_colors(&self, connector: Connector, color_space: ColorSpace, eotf: Eotf) {
        self.send(&ClientMessage::ConnectorSetColors {
            connector,
            color_space,
            eotf,
        });
    }

    pub fn connector_set_blend_space(&self, connector: Connector, blend_space: BlendSpace) {
        self.send(&ClientMessage::ConnectorSetBlendSpace {
            connector,
            blend_space,
        });
    }

    pub fn connector_set_brightness(&self, connector: Connector, brightness: Option<f64>) {
        self.send(&ClientMessage::ConnectorSetBrightness {
            connector,
            brightness,
        });
    }

    pub fn connector_get_scale(&self, connector: Connector) -> f64 {
        let res = self.send_with_response(&ClientMessage::ConnectorGetScale { connector });
        get_response!(res, 1.0, ConnectorGetScale { scale });
        scale
    }

    pub fn connector_type(&self, connector: Connector) -> ConnectorType {
        let res = self.send_with_response(&ClientMessage::ConnectorType { connector });
        get_response!(res, CON_UNKNOWN, ConnectorType { ty });
        ty
    }

    pub fn connector_mode(&self, connector: Connector) -> Mode {
        let res = self.send_with_response(&ClientMessage::ConnectorMode { connector });
        get_response!(
            res,
            Mode::zeroed(),
            ConnectorMode {
                width,
                height,
                refresh_millihz
            }
        );
        Mode {
            width,
            height,
            refresh_millihz,
        }
    }

    pub fn connector_set_mode(&self, connector: Connector, mode: WireMode) {
        self.send(&ClientMessage::ConnectorSetMode { connector, mode });
    }

    pub fn connector_modes(&self, connector: Connector) -> Vec<Mode> {
        let res = self.send_with_response(&ClientMessage::ConnectorModes { connector });
        get_response!(res, Vec::new(), ConnectorModes { modes });
        modes.into_iter().map(WireMode::to_mode).collect()
    }

    pub fn connector_size(&self, connector: Connector) -> (i32, i32) {
        let res = self.send_with_response(&ClientMessage::ConnectorSize { connector });
        get_response!(res, (0, 0), ConnectorSize { width, height });
        (width, height)
    }

    pub fn set_x_scaling_mode(&self, mode: XScalingMode) {
        self.send(&ClientMessage::SetXScalingMode { mode })
    }

    pub fn set_vrr_mode(&self, connector: Option<Connector>, mode: VrrMode) {
        self.send(&ClientMessage::SetVrrMode { connector, mode })
    }

    pub fn set_vrr_cursor_hz(&self, connector: Option<Connector>, hz: f64) {
        self.send(&ClientMessage::SetVrrCursorHz { connector, hz })
    }

    pub fn set_tearing_mode(&self, connector: Option<Connector>, mode: TearingMode) {
        self.send(&ClientMessage::SetTearingMode { connector, mode })
    }

    pub fn drm_devices(&self) -> Vec<DrmDevice> {
        let res = self.send_with_response(&ClientMessage::GetDrmDevices);
        get_response!(res, vec![], GetDrmDevices { devices });
        devices
    }

    pub fn on_new_drm_device<F: FnMut(DrmDevice) + 'static>(&self, f: F) {
        *self.on_new_drm_device.borrow_mut() = Some(cb(f));
    }

    pub fn on_del_drm_device<F: FnMut(DrmDevice) + 'static>(&self, f: F) {
        *self.on_del_drm_device.borrow_mut() = Some(cb(f));
    }

    pub fn on_new_connector<F: FnMut(Connector) + 'static>(&self, f: F) {
        *self.on_new_connector.borrow_mut() = Some(cb(f));
    }

    pub fn on_idle<F: FnMut() + 'static>(&self, mut f: F) {
        *self.on_idle.borrow_mut() = Some(cb(move |_| f()));
    }

    pub fn on_connector_connected<F: FnMut(Connector) + 'static>(&self, f: F) {
        *self.on_connector_connected.borrow_mut() = Some(cb(f));
    }

    pub fn on_connector_disconnected<F: FnMut(Connector) + 'static>(&self, f: F) {
        *self.on_connector_disconnected.borrow_mut() = Some(cb(f));
    }

    pub fn on_graphics_initialized<F: FnOnce() + 'static>(&self, f: F) {
        self.on_graphics_initialized.set(Some(Box::new(f)));
    }

    pub fn on_devices_enumerated<F: FnOnce() + 'static>(&self, f: F) {
        self.on_devices_enumerated.set(Some(Box::new(f)));
    }

    pub fn config_dir(&self) -> String {
        let res = self.send_with_response(&ClientMessage::GetConfigDir);
        get_response!(res, String::new(), GetConfigDir { dir });
        dir
    }

    pub fn workspaces(&self) -> Vec<Workspace> {
        let res = self.send_with_response(&ClientMessage::GetWorkspaces);
        get_response!(res, vec![], GetWorkspaces { workspaces });
        workspaces
    }

    pub fn set_idle(&self, timeout: Duration) {
        self.send(&ClientMessage::SetIdle { timeout })
    }

    pub fn set_idle_grace_period(&self, period: Duration) {
        self.send(&ClientMessage::SetIdleGracePeriod { period })
    }

    pub fn set_explicit_sync_enabled(&self, enabled: bool) {
        self.send(&ClientMessage::SetExplicitSyncEnabled { enabled })
    }

    pub fn set_seat(&self, device: InputDevice, seat: Seat) {
        self.send(&ClientMessage::SetSeat { device, seat })
    }

    pub fn set_device_keymap(&self, device: InputDevice, keymap: Keymap) {
        self.send(&ClientMessage::DeviceSetKeymap { device, keymap })
    }

    pub fn set_left_handed(&self, device: InputDevice, left_handed: bool) {
        self.send(&ClientMessage::SetLeftHanded {
            device,
            left_handed,
        })
    }

    pub fn set_accel_profile(&self, device: InputDevice, profile: AccelProfile) {
        self.send(&ClientMessage::SetAccelProfile { device, profile })
    }

    pub fn set_accel_speed(&self, device: InputDevice, speed: f64) {
        self.send(&ClientMessage::SetAccelSpeed { device, speed })
    }

    pub fn set_transform_matrix(&self, device: InputDevice, matrix: [[f64; 2]; 2]) {
        self.send(&ClientMessage::SetTransformMatrix { device, matrix })
    }

    pub fn set_calibration_matrix(&self, device: InputDevice, matrix: [[f32; 3]; 2]) {
        self.send(&ClientMessage::SetCalibrationMatrix { device, matrix })
    }

    pub fn set_px_per_wheel_scroll(&self, device: InputDevice, px: f64) {
        self.send(&ClientMessage::SetPxPerWheelScroll { device, px })
    }

    pub fn set_input_tap_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetTapEnabled { device, enabled })
    }

    pub fn set_input_natural_scrolling_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetNaturalScrollingEnabled { device, enabled })
    }

    pub fn set_input_drag_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetDragEnabled { device, enabled })
    }

    pub fn set_input_drag_lock_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetDragLockEnabled { device, enabled })
    }

    pub fn set_input_click_method(&self, device: InputDevice, method: ClickMethod) {
        self.send(&ClientMessage::SetClickMethod { device, method })
    }

    pub fn set_input_middle_button_emulation_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetMiddleButtonEmulationEnabled { device, enabled })
    }

    pub fn device_name(&self, device: InputDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDeviceName { device });
        get_response!(res, String::new(), GetDeviceName { name });
        name
    }

    pub fn input_device_syspath(&self, device: InputDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetInputDeviceSyspath { device });
        get_response!(res, String::new(), GetInputDeviceSyspath { syspath });
        syspath
    }

    pub fn input_device_devnode(&self, device: InputDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetInputDeviceDevnode { device });
        get_response!(res, String::new(), GetInputDeviceDevnode { devnode });
        devnode
    }

    pub fn has_capability(&self, device: InputDevice, cap: Capability) -> bool {
        let res = self.send_with_response(&ClientMessage::HasCapability { device, cap });
        get_response!(res, false, HasCapability { has });
        has
    }

    pub fn destroy_keymap(&self, keymap: Keymap) {
        self.send(&ClientMessage::DestroyKeymap { keymap })
    }

    pub fn seat_set_keymap(&self, seat: Seat, keymap: Keymap) {
        self.send(&ClientMessage::SeatSetKeymap { seat, keymap })
    }

    pub fn seat_set_repeat_rate(&self, seat: Seat, rate: i32, delay: i32) {
        self.send(&ClientMessage::SeatSetRepeatRate { seat, rate, delay })
    }

    pub fn seat_get_repeat_rate(&self, seat: Seat) -> (i32, i32) {
        let res = self.send_with_response(&ClientMessage::SeatGetRepeatRate { seat });
        get_response!(res, (25, 250), GetRepeatRate { rate, delay });
        (rate, delay)
    }

    pub fn set_forward(&self, seat: Seat, forward: bool) {
        self.send(&ClientMessage::SetForward { seat, forward })
    }

    pub fn set_focus_follows_mouse_mode(&self, seat: Seat, mode: FocusFollowsMouseMode) {
        self.send(&ClientMessage::SetFocusFollowsMouseMode { seat, mode })
    }

    pub fn set_window_management_enabled(&self, seat: Seat, enabled: bool) {
        self.send(&ClientMessage::SetWindowManagementEnabled { seat, enabled })
    }

    pub fn set_input_device_connector(&self, input_device: InputDevice, connector: Connector) {
        self.send(&ClientMessage::SetInputDeviceConnector {
            input_device,
            connector,
        })
    }

    pub fn remove_input_mapping(&self, input_device: InputDevice) {
        self.send(&ClientMessage::RemoveInputMapping { input_device })
    }

    pub fn parse_keymap(&self, keymap: &str) -> Keymap {
        let res = self.send_with_response(&ClientMessage::ParseKeymap { keymap });
        get_response!(res, Keymap(0), ParseKeymap { keymap });
        keymap
    }

    pub fn set_ei_socket_enabled(&self, enabled: bool) {
        self.send(&ClientMessage::SetEiSocketEnabled { enabled })
    }

    pub fn get_connector_active_workspace(&self, connector: Connector) -> Workspace {
        let res =
            self.send_with_response(&ClientMessage::GetConnectorActiveWorkspace { connector });
        get_response!(res, Workspace(0), GetConnectorActiveWorkspace { workspace });
        workspace
    }

    pub fn get_connector_workspaces(&self, connector: Connector) -> Vec<Workspace> {
        let res = self.send_with_response(&ClientMessage::GetConnectorWorkspaces { connector });
        get_response!(res, vec![], GetConnectorWorkspaces { workspaces });
        workspaces
    }

    pub fn set_client_matcher_capabilities(
        &self,
        matcher: ClientMatcher,
        caps: ClientCapabilities,
    ) {
        self.send(&ClientMessage::SetClientMatcherCapabilities { matcher, caps });
    }

    pub fn set_client_matcher_bounding_capabilities(
        &self,
        matcher: ClientMatcher,
        caps: ClientCapabilities,
    ) {
        self.send(&ClientMessage::SetClientMatcherBoundingCapabilities { matcher, caps });
    }

    pub fn latch<F: FnOnce() + 'static>(&self, seat: Seat, f: F) {
        if !self.feat_mod_mask.get() {
            log::error!("compositor does not support latching");
            return;
        }
        let Some(keysym) = self.pressed_keysym.get() else {
            log::error!("latch called while not executing shortcut");
            return;
        };
        let mods = RELEASE;
        let f = Box::new(f);
        let register = {
            let mut kh = self.key_handlers.borrow_mut();
            match kh.entry((seat, mods | keysym)) {
                Entry::Occupied(mut o) => {
                    let o = o.get_mut();
                    o.latched.push(f);
                    mem::replace(&mut o.registered_mask, mods) != mods
                }
                Entry::Vacant(v) => {
                    v.insert(KeyHandler {
                        cb_mask: mods,
                        registered_mask: mods,
                        cb: None,
                        latched: vec![f],
                    });
                    true
                }
            }
        };
        if register {
            self.send(&ClientMessage::AddShortcut2 {
                seat,
                mods,
                mod_mask: mods,
                sym: keysym,
            });
        }
    }

    pub fn bind_masked<F: FnMut() + 'static>(
        &self,
        seat: Seat,
        mut mod_mask: Modifiers,
        mod_sym: ModifiedKeySym,
        mut f: F,
    ) {
        mod_mask |= mod_sym.mods | RELEASE;
        let register = {
            let mut kh = self.key_handlers.borrow_mut();
            let cb = cb(move |_| f());
            match kh.entry((seat, mod_sym)) {
                Entry::Occupied(mut o) => {
                    let o = o.get_mut();
                    o.cb = Some(cb);
                    o.cb_mask = mod_mask;
                    let register = o.latched.is_empty() && o.registered_mask != o.cb_mask;
                    if register {
                        o.registered_mask = o.cb_mask;
                    }
                    register
                }
                Entry::Vacant(v) => {
                    v.insert(KeyHandler {
                        cb_mask: mod_mask,
                        registered_mask: mod_mask,
                        cb: Some(cb),
                        latched: vec![],
                    });
                    true
                }
            }
        };
        if register {
            let msg = if self.feat_mod_mask.get() {
                ClientMessage::AddShortcut2 {
                    seat,
                    mods: mod_sym.mods,
                    mod_mask,
                    sym: mod_sym.sym,
                }
            } else {
                ClientMessage::AddShortcut {
                    seat,
                    mods: mod_sym.mods,
                    sym: mod_sym.sym,
                }
            };
            self.send(&msg);
        }
    }

    pub fn log(&self, level: LogLevel, msg: &str, file: Option<&str>, line: Option<u32>) {
        self.send(&ClientMessage::Log {
            level,
            msg,
            file,
            line,
        })
    }

    pub fn get_socket_path(&self) -> Option<String> {
        let res = self.send_with_response(&ClientMessage::GetSocketPath);
        get_response!(res, None, GetSocketPath { path });
        Some(path)
    }

    pub fn create_pollable(&self, fd: i32) -> Result<PollableId, String> {
        let res = self.send_with_response(&ClientMessage::AddPollable { fd });
        get_response!(
            res,
            Err("Compositor did not send a response".to_string()),
            AddPollable { id }
        );
        id
    }

    pub fn remove_pollable(&self, id: PollableId) {
        self.send(&ClientMessage::RemovePollable { id });
        self.write_interests.borrow_mut().remove(&id);
        self.read_interests.borrow_mut().remove(&id);
    }

    pub fn poll_io(
        &self,
        pollable: PollableId,
        writable: bool,
        ctx: &mut Context<'_>,
    ) -> Poll<Result<(), String>> {
        let interests = match writable {
            true => &self.write_interests,
            false => &self.read_interests,
        };
        let mut interests = interests.borrow_mut();
        match interests.entry(pollable) {
            Entry::Occupied(mut o) => {
                let interest = o.get_mut();
                if interest.result.is_some() {
                    Poll::Ready(o.remove().result.unwrap())
                } else {
                    interest.waker = Some(ctx.waker().clone());
                    Poll::Pending
                }
            }
            Entry::Vacant(v) => {
                self.send(&ClientMessage::AddInterest { pollable, writable });
                v.insert(Interest {
                    result: None,
                    waker: Some(ctx.waker().clone()),
                });
                Poll::Pending
            }
        }
    }

    pub fn on_unload(&self, f: impl FnOnce() + 'static) {
        if let Some(prev) = self.on_unload.replace(Some(on_drop(Box::new(f)))) {
            prev.forget();
        }
    }

    pub fn clients(&self) -> Vec<Client> {
        let res = self.send_with_response(&ClientMessage::GetClients);
        get_response!(res, vec!(), GetClients { clients });
        clients
    }

    pub fn client_exists(&self, client: Client) -> bool {
        let res = self.send_with_response(&ClientMessage::ClientExists { client });
        get_response!(res, false, ClientExists { exists });
        exists
    }

    pub fn client_is_xwayland(&self, client: Client) -> bool {
        let res = self.send_with_response(&ClientMessage::ClientIsXwayland { client });
        get_response!(res, false, ClientIsXwayland { is_xwayland });
        is_xwayland
    }

    pub fn client_kill(&self, client: Client) {
        self.send(&ClientMessage::ClientKill { client });
    }

    fn create_generic_matcher<Crit, Matcher>(
        &self,
        criterion: GenericCriterion<'_, Crit, Matcher>,
        child: bool,
        create_child_matcher: impl Fn(Crit) -> (Matcher, bool),
        create_matcher: impl Fn(GenericCriterionIpc<Matcher>) -> Matcher,
        destroy_matcher: impl Fn(Matcher),
    ) -> (Matcher, bool)
    where
        Crit: Copy,
        Matcher: Copy,
    {
        let mut ad_hoc = vec![];
        let mut create_child_matcher = |c: Crit| {
            let (m, original) = create_child_matcher(c);
            if original {
                ad_hoc.push(m);
            }
            m
        };
        let mut create_vec = |l: &[Crit]| {
            let mut list = Vec::with_capacity(l.len());
            for c in l {
                list.push(create_child_matcher(*c));
            }
            list
        };
        let criterion = match criterion {
            GenericCriterion::Matcher(m) => {
                if child {
                    return (m, false);
                }
                GenericCriterionIpc::Matcher(m)
            }
            GenericCriterion::Not(c) => GenericCriterionIpc::Not(create_child_matcher(*c)),
            GenericCriterion::All(l) => GenericCriterionIpc::List {
                list: create_vec(l),
                all: true,
            },
            GenericCriterion::Any(l) => GenericCriterionIpc::List {
                list: create_vec(l),
                all: false,
            },
            GenericCriterion::Exactly(num, l) => GenericCriterionIpc::Exactly {
                list: create_vec(l),
                num,
            },
        };
        let matcher = create_matcher(criterion);
        for matcher in ad_hoc {
            destroy_matcher(matcher);
        }
        (matcher, true)
    }

    pub fn create_client_matcher(&self, criterion: ClientCriterion<'_>) -> ClientMatcher {
        self.create_client_matcher_(criterion, false).0
    }

    fn create_client_matcher_(
        &self,
        criterion: ClientCriterion<'_>,
        child: bool,
    ) -> (ClientMatcher, bool) {
        macro_rules! string {
            ($t:expr, $field:ident, $regex:expr) => {
                ClientCriterionIpc::String {
                    string: $t.to_string(),
                    field: ClientCriterionStringField::$field,
                    regex: $regex,
                }
            };
        }
        let create_matcher = |criterion| {
            let res = self.send_with_response(&ClientMessage::CreateClientMatcher {
                criterion: ClientCriterionIpc::Generic(criterion),
            });
            get_response!(res, ClientMatcher(0), CreateClientMatcher { matcher });
            matcher
        };
        let destroy_matcher = |matcher| {
            self.send(&ClientMessage::DestroyClientMatcher { matcher });
        };
        let generic = |crit: GenericCriterion<ClientCriterion, ClientMatcher>| {
            self.create_generic_matcher::<ClientCriterion, ClientMatcher>(
                crit,
                child,
                |c| self.create_client_matcher_(c, true),
                create_matcher,
                destroy_matcher,
            )
        };
        let criterion = match criterion {
            ClientCriterion::Matcher(m) => return generic(GenericCriterion::Matcher(m)),
            ClientCriterion::Not(c) => return generic(GenericCriterion::Not(c)),
            ClientCriterion::All(c) => return generic(GenericCriterion::All(c)),
            ClientCriterion::Any(c) => return generic(GenericCriterion::Any(c)),
            ClientCriterion::Exactly(n, c) => return generic(GenericCriterion::Exactly(n, c)),
            ClientCriterion::SandboxEngine(t) => string!(t, SandboxEngine, false),
            ClientCriterion::SandboxEngineRegex(t) => string!(t, SandboxEngine, true),
            ClientCriterion::SandboxAppId(t) => string!(t, SandboxAppId, false),
            ClientCriterion::SandboxAppIdRegex(t) => string!(t, SandboxAppId, true),
            ClientCriterion::SandboxInstanceId(t) => string!(t, SandboxInstanceId, false),
            ClientCriterion::SandboxInstanceIdRegex(t) => string!(t, SandboxInstanceId, true),
            ClientCriterion::Sandboxed => ClientCriterionIpc::Sandboxed,
            ClientCriterion::Uid(p) => ClientCriterionIpc::Uid(p),
            ClientCriterion::Pid(p) => ClientCriterionIpc::Pid(p),
            ClientCriterion::IsXwayland => ClientCriterionIpc::IsXwayland,
            ClientCriterion::Comm(t) => string!(t, Comm, false),
            ClientCriterion::CommRegex(t) => string!(t, Comm, true),
            ClientCriterion::Exe(t) => string!(t, Exe, false),
            ClientCriterion::ExeRegex(t) => string!(t, Exe, true),
        };
        let res = self.send_with_response(&ClientMessage::CreateClientMatcher { criterion });
        get_response!(
            res,
            (ClientMatcher(0), false),
            CreateClientMatcher { matcher }
        );
        (matcher, true)
    }

    pub fn set_client_matcher_handler(
        &self,
        matcher: ClientMatcher,
        cb: impl FnMut(MatchedClient) + 'static,
    ) {
        let cb = Rc::new(RefCell::new(cb));
        let handlers = &mut *self.client_match_handlers.borrow_mut();
        let handler = handlers.entry(matcher).or_insert_with(|| {
            self.send(&ClientMessage::EnableClientMatcherEvents { matcher });
            ClientMatchHandler {
                cb: cb.clone(),
                latched: Default::default(),
            }
        });
        handler.cb = cb.clone();
    }

    pub fn set_client_matcher_latch_handler(
        &self,
        matcher: ClientMatcher,
        client: Client,
        cb: impl FnOnce() + 'static,
    ) {
        let handlers = &mut *self.client_match_handlers.borrow_mut();
        if let Some(handler) = handlers.get_mut(&matcher) {
            handler.latched.insert(client, Box::new(cb));
        }
    }

    pub fn destroy_client_matcher(&self, matcher: ClientMatcher) {
        self.send(&ClientMessage::DestroyClientMatcher { matcher });
        self.client_match_handlers.borrow_mut().remove(&matcher);
    }

    pub fn create_window_matcher(&self, criterion: WindowCriterion) -> WindowMatcher {
        self.create_window_matcher_(criterion, false).0
    }

    fn create_window_matcher_(
        &self,
        criterion: WindowCriterion,
        child: bool,
    ) -> (WindowMatcher, bool) {
        macro_rules! string {
            ($t:expr, $field:ident, $regex:expr) => {
                WindowCriterionIpc::String {
                    string: $t.to_string(),
                    field: WindowCriterionStringField::$field,
                    regex: $regex,
                }
            };
        }
        let create_matcher = |criterion| {
            let res = self.send_with_response(&ClientMessage::CreateWindowMatcher {
                criterion: WindowCriterionIpc::Generic(criterion),
            });
            get_response!(res, WindowMatcher(0), CreateWindowMatcher { matcher });
            matcher
        };
        let destroy_matcher = |matcher| {
            self.send(&ClientMessage::DestroyWindowMatcher { matcher });
        };
        let generic = |crit: GenericCriterion<WindowCriterion, WindowMatcher>| {
            self.create_generic_matcher(
                crit,
                child,
                |c| self.create_window_matcher_(c, true),
                create_matcher,
                destroy_matcher,
            )
        };
        let _destroy_client_matcher;
        let criterion = match criterion {
            WindowCriterion::Matcher(m) => return generic(GenericCriterion::Matcher(m)),
            WindowCriterion::Not(c) => return generic(GenericCriterion::Not(c)),
            WindowCriterion::All(c) => return generic(GenericCriterion::All(c)),
            WindowCriterion::Any(c) => return generic(GenericCriterion::Any(c)),
            WindowCriterion::Exactly(n, c) => return generic(GenericCriterion::Exactly(n, c)),
            WindowCriterion::Types(t) => WindowCriterionIpc::Types(t),
            WindowCriterion::Client(c) => {
                let (matcher, original) = self.create_client_matcher_(*c, true);
                if original {
                    _destroy_client_matcher = on_drop(move || matcher.destroy());
                }
                WindowCriterionIpc::Client(matcher)
            }
            WindowCriterion::Title(t) => string!(t, Title, false),
            WindowCriterion::TitleRegex(t) => string!(t, Title, true),
            WindowCriterion::AppId(t) => string!(t, AppId, false),
            WindowCriterion::AppIdRegex(t) => string!(t, AppId, true),
            WindowCriterion::Floating => WindowCriterionIpc::Floating,
            WindowCriterion::Visible => WindowCriterionIpc::Visible,
            WindowCriterion::Urgent => WindowCriterionIpc::Urgent,
            WindowCriterion::Focus(seat) => WindowCriterionIpc::SeatFocus(seat),
            WindowCriterion::Fullscreen => WindowCriterionIpc::Fullscreen,
            WindowCriterion::JustMapped => WindowCriterionIpc::JustMapped,
            WindowCriterion::Tag(t) => string!(t, Tag, false),
            WindowCriterion::TagRegex(t) => string!(t, Tag, true),
            WindowCriterion::XClass(t) => string!(t, XClass, false),
            WindowCriterion::XClassRegex(t) => string!(t, XClass, true),
            WindowCriterion::XInstance(t) => string!(t, XInstance, false),
            WindowCriterion::XInstanceRegex(t) => string!(t, XInstance, true),
            WindowCriterion::XRole(t) => string!(t, XRole, false),
            WindowCriterion::XRoleRegex(t) => string!(t, XRole, true),
            WindowCriterion::Workspace(t) => WindowCriterionIpc::Workspace(t),
            WindowCriterion::WorkspaceName(t) => string!(t, Workspace, false),
            WindowCriterion::WorkspaceNameRegex(t) => string!(t, Workspace, true),
            WindowCriterion::ContentTypes(t) => WindowCriterionIpc::ContentTypes(t),
        };
        let res = self.send_with_response(&ClientMessage::CreateWindowMatcher { criterion });
        get_response!(
            res,
            (WindowMatcher(0), false),
            CreateWindowMatcher { matcher }
        );
        (matcher, true)
    }

    pub fn set_window_matcher_handler(
        &self,
        matcher: WindowMatcher,
        cb: impl FnMut(MatchedWindow) + 'static,
    ) {
        let cb = Rc::new(RefCell::new(cb));
        let handlers = &mut *self.window_match_handlers.borrow_mut();
        let handler = handlers.entry(matcher).or_insert_with(|| {
            self.send(&ClientMessage::EnableWindowMatcherEvents { matcher });
            WindowMatchHandler {
                cb: cb.clone(),
                latched: Default::default(),
            }
        });
        handler.cb = cb.clone();
    }

    pub fn set_window_matcher_auto_focus(&self, matcher: WindowMatcher, auto_focus: bool) {
        self.send(&ClientMessage::SetWindowMatcherAutoFocus {
            matcher,
            auto_focus,
        });
    }

    pub fn set_window_matcher_initial_tile_state(
        &self,
        matcher: WindowMatcher,
        tile_state: TileState,
    ) {
        self.send(&ClientMessage::SetWindowMatcherInitialTileState {
            matcher,
            tile_state,
        });
    }

    pub fn set_window_matcher_latch_handler(
        &self,
        matcher: WindowMatcher,
        window: Window,
        cb: impl FnOnce() + 'static,
    ) {
        let handlers = &mut *self.window_match_handlers.borrow_mut();
        if let Some(handler) = handlers.get_mut(&matcher) {
            handler.latched.insert(window, Box::new(cb));
        }
    }

    pub fn destroy_window_matcher(&self, matcher: WindowMatcher) {
        self.send(&ClientMessage::DestroyWindowMatcher { matcher });
        self.window_match_handlers.borrow_mut().remove(&matcher);
    }

    pub fn set_pointer_revert_key(&self, seat: Seat, key: KeySym) {
        self.send(&ClientMessage::SetPointerRevertKey { seat, key });
    }

    fn handle_msg(&self, msg: &[u8]) {
        self.handle_msg2(msg);
        self.dispatch_futures();
    }

    fn dispatch_futures(&self) {
        let futures = &self.tasks;
        if !futures.ready_back.any.load(Relaxed) {
            return;
        }
        let mut ready = futures.ready_front.take();
        loop {
            mem::swap(&mut ready, &mut *futures.ready_back.tasks.lock().unwrap());
            futures.ready_back.any.store(false, Relaxed);
            while let Some(id) = ready.pop_front() {
                let fut = futures.tasks.borrow_mut().get(&id).cloned();
                if let Some(fut) = fut {
                    let mut fut = fut.borrow_mut();
                    let fut = &mut *fut;
                    let res = catch_unwind(AssertUnwindSafe(|| {
                        fut.task.as_mut().poll(&mut Context::from_waker(&fut.waker))
                    }));
                    match res {
                        Err(_) => {
                            log::error!("A task panicked");
                            let _tmp = futures.tasks.borrow_mut().remove(&id);
                        }
                        Ok(Poll::Ready(())) => {
                            let _tmp = futures.tasks.borrow_mut().remove(&id);
                        }
                        Ok(_) => {}
                    }
                }
            }
            if !futures.ready_back.any.load(Relaxed) {
                break;
            }
        }
        futures.ready_front.set(ready);
    }

    pub fn spawn_task<T: 'static>(&self, f: impl Future<Output = T> + 'static) -> Rc<JoinSlot<T>> {
        struct Waker(Arc<TasksBackBuffer>, u64);
        impl ArcWake for Waker {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.0.append(arc_self.1);
            }
        }
        let tasks = &self.tasks;
        let id = tasks.last_id.get() + 1;
        tasks.last_id.set(id);
        let waker = futures_util::task::waker(Arc::new(Waker(tasks.ready_back.clone(), id)));
        tasks.ready_back.append(id);
        let slot = Rc::new(JoinSlot {
            task_id: id,
            slot: Cell::new(None),
            waker: Cell::new(None),
        });
        let slot2 = slot.clone();
        let task = Rc::new(RefCell::new(Task {
            task: Box::pin(async move {
                slot2.slot.set(Some(f.await));
                if let Some(waker) = slot2.waker.take() {
                    waker.wake();
                }
            }),
            waker,
        }));
        tasks.tasks.borrow_mut().insert(id, task);
        slot
    }

    pub fn abort_task(&self, id: u64) {
        let _tmp = self.tasks.tasks.borrow_mut().remove(&id);
    }

    fn handle_invoke_shortcut(
        &self,
        seat: Seat,
        unmasked_mods: Modifiers,
        mods: Modifiers,
        sym: KeySym,
    ) {
        let ms = ModifiedKeySym { mods, sym };
        let handler = self
            .key_handlers
            .borrow_mut()
            .get_mut(&(seat, ms))
            .map(|kh| {
                let cb = if kh.cb_mask & unmasked_mods == mods {
                    kh.cb.clone()
                } else {
                    None
                };
                (mem::take(&mut kh.latched), cb)
            });
        let Some((latched, handler)) = handler else {
            return;
        };
        let was_latched = !latched.is_empty();
        if (mods & RELEASE).0 == 0 {
            self.pressed_keysym.set(Some(sym));
        }
        for latched in latched {
            ignore_panic("latch", latched);
        }
        if let Some(handler) = handler {
            run_cb("shortcut", &handler, ());
        }
        self.pressed_keysym.set(None);
        if was_latched
            && let Entry::Occupied(mut oe) = self.key_handlers.borrow_mut().entry((seat, ms))
        {
            let o = oe.get_mut();
            if o.latched.is_empty() {
                if o.cb.is_none() {
                    self.send(&ClientMessage::RemoveShortcut { seat, mods, sym });
                    oe.remove();
                } else if o.cb_mask != o.registered_mask {
                    o.registered_mask = o.cb_mask;
                    self.send(&ClientMessage::AddShortcut2 {
                        seat,
                        mods: ms.mods,
                        mod_mask: o.cb_mask,
                        sym: ms.sym,
                    });
                }
            }
        }
    }

    fn handle_msg2(&self, msg: &[u8]) {
        let res = bincode_ops().deserialize::<ServerMessage>(msg);
        let msg = match res {
            Ok(msg) => msg,
            Err(e) => {
                let msg = format!("could not deserialize message: {}", e);
                self.log(LogLevel::Error, &msg, None, None);
                return;
            }
        };
        match msg {
            ServerMessage::Configure { reload } => {
                self.reload.set(reload);
                (self.configure)();
                self.reload.set(false);
            }
            ServerMessage::Response { response } => {
                self.response.borrow_mut().push(response);
            }
            ServerMessage::InvokeShortcut { seat, mods, sym } => {
                self.handle_invoke_shortcut(seat, mods, mods, sym);
            }
            ServerMessage::InvokeShortcut2 {
                seat,
                unmasked_mods,
                effective_mods,
                sym,
            } => {
                self.handle_invoke_shortcut(seat, unmasked_mods, effective_mods, sym);
            }
            ServerMessage::NewInputDevice { device } => {
                let handler = self.on_new_input_device.borrow_mut().clone();
                if let Some(handler) = handler {
                    run_cb("new input device", &handler, device);
                }
            }
            ServerMessage::DelInputDevice { device } => {
                self.on_switch_event.borrow_mut().remove(&device);
                let handler = self.on_input_device_removed.borrow_mut().clone();
                if let Some(handler) = handler {
                    run_cb("input device removed", &handler, device);
                }
            }
            ServerMessage::ConnectorConnect { device } => {
                let handler = self.on_connector_connected.borrow_mut().clone();
                if let Some(handler) = handler {
                    run_cb("connector connected", &handler, device);
                }
            }
            ServerMessage::ConnectorDisconnect { device } => {
                let handler = self.on_connector_disconnected.borrow_mut().clone();
                if let Some(handler) = handler {
                    run_cb("connector disconnected", &handler, device);
                }
            }
            ServerMessage::NewConnector { device } => {
                let handler = self.on_new_connector.borrow_mut().clone();
                if let Some(handler) = handler {
                    run_cb("new connector", &handler, device);
                }
            }
            ServerMessage::DelConnector { .. } => {}
            ServerMessage::TimerExpired { timer } => {
                let handler = self.timer_handlers.borrow_mut().get(&timer).cloned();
                if let Some(handler) = handler {
                    run_cb("timer", &handler, ());
                }
            }
            ServerMessage::GraphicsInitialized => {
                if let Some(handler) = self.on_graphics_initialized.take() {
                    ignore_panic("graphics initialized", handler);
                }
            }
            ServerMessage::Clear => {
                // only used by test config
            }
            ServerMessage::NewDrmDev { device } => {
                let handler = self.on_new_drm_device.borrow_mut();
                if let Some(handler) = handler.deref() {
                    run_cb("new drm device", handler, device);
                }
            }
            ServerMessage::DelDrmDev { device } => {
                let handler = self.on_del_drm_device.borrow_mut();
                if let Some(handler) = handler.deref() {
                    run_cb("del drm device", handler, device);
                }
            }
            ServerMessage::Idle => {
                let handler = self.on_idle.borrow_mut();
                if let Some(handler) = handler.deref() {
                    run_cb("idle", handler, ());
                }
            }
            ServerMessage::DevicesEnumerated => {
                if let Some(handler) = self.on_devices_enumerated.take() {
                    ignore_panic("devices enumerated", handler);
                }
            }
            ServerMessage::InterestReady { id, writable, res } => {
                let interests = match writable {
                    true => &self.write_interests,
                    false => &self.read_interests,
                };
                let mut interests = interests.borrow_mut();
                if let Some(interest) = interests.get_mut(&id) {
                    interest.result = Some(res);
                    if let Some(waker) = interest.waker.take() {
                        waker.wake();
                    }
                }
            }
            ServerMessage::Features { features } => {
                for feat in features {
                    match feat {
                        ServerFeature::NONE => {}
                        ServerFeature::MOD_MASK => self.feat_mod_mask.set(true),
                        _ => {}
                    }
                }
            }
            ServerMessage::SwitchEvent {
                seat,
                input_device,
                event,
            } => {
                let _ = seat;
                let cb = self.on_switch_event.borrow().get(&input_device).cloned();
                if let Some(cb) = cb {
                    run_cb("switch event", &cb, event);
                }
            }
            ServerMessage::ClientMatcherMatched { matcher, client } => {
                let cb = {
                    let handlers = self.client_match_handlers.borrow();
                    let Some(handler) = handlers.get(&matcher) else {
                        return;
                    };
                    handler.cb.clone()
                };
                let matched = MatchedClient { matcher, client };
                cb.borrow_mut()(matched);
            }
            ServerMessage::ClientMatcherUnmatched { matcher, client } => {
                let cb = {
                    let mut handlers = self.client_match_handlers.borrow_mut();
                    let Some(handler) = handlers.get_mut(&matcher) else {
                        return;
                    };
                    let Some(cb) = handler.latched.remove(&client) else {
                        return;
                    };
                    cb
                };
                cb();
            }
            ServerMessage::WindowMatcherMatched { matcher, window } => {
                let cb = {
                    let handlers = self.window_match_handlers.borrow();
                    let Some(handler) = handlers.get(&matcher) else {
                        return;
                    };
                    handler.cb.clone()
                };
                let matched = MatchedWindow { matcher, window };
                cb.borrow_mut()(matched);
            }
            ServerMessage::WindowMatcherUnmatched { matcher, window } => {
                let cb = {
                    let mut handlers = self.window_match_handlers.borrow_mut();
                    let Some(handler) = handlers.get_mut(&matcher) else {
                        return;
                    };
                    let Some(cb) = handler.latched.remove(&window) else {
                        return;
                    };
                    cb
                };
                cb();
            }
        }
    }

    fn handle_init_msg(&self, msg: &[u8]) {
        let init = match bincode_ops().deserialize::<InitMessage>(msg) {
            Ok(m) => m,
            Err(e) => {
                let msg = format!("could not deserialize message: {}", e);
                self.log(LogLevel::Error, &msg, None, None);
                return;
            }
        };
        match init {
            InitMessage::V1(_) => {}
        }
    }
}

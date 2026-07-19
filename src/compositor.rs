use crate::acceptor::Acceptor;
use crate::acceptor::AcceptorError;
use crate::async_engine::AsyncEngine;
use crate::async_engine::Phase;
use crate::async_engine::SpawnedFuture;
use crate::backend::Backend;
use crate::backend::Connector;
use crate::backends::dummy::DummyBackend;
use crate::backends::dummy::DummyOutput;
use crate::backends::headless;
use crate::backends::metal;
use crate::backends::x;
use crate::cli::CliBackend;
use crate::cli::GlobalArgs;
use crate::cli::RunArgs;
use crate::client::ClientId;
use crate::client::Clients;
use crate::clientmem::ClientMemError;
use crate::clientmem::{self};
use crate::cmm::cmm_manager::ColorManager;
use crate::cmm::cmm_primaries::Primaries;
use crate::config::ConfigProxy;
use crate::configurable::handle_configurables_apply;
use crate::configurable::handle_configurables_timeout;
use crate::control_center::redraw_control_centers;
use crate::copy_device::CopyDeviceRegistry;
use crate::cpu_worker::CpuWorker;
use crate::cpu_worker::CpuWorkerError;
use crate::criteria::CritMatcherIds;
use crate::criteria::clm::ClMatcherManager;
use crate::criteria::clm::handle_cl_changes;
use crate::criteria::clm::handle_cl_leaf_events;
use crate::criteria::tlm::TlMatcherManager;
use crate::criteria::tlm::handle_tl_changes;
use crate::criteria::tlm::handle_tl_just_mapped;
use crate::criteria::tlm::handle_tl_leaf_events;
use crate::damage::DamageVisualizer;
use crate::damage::visualize_damage;
use crate::dbus::Dbus;
use crate::dmabuf_feedback::handle_dmabuf_feedback_changes;
use crate::ei::ei_client::EiClients;
use crate::eventfd_cache::EventfdCache;
use crate::forker;
use crate::format::XRGB8888;
use crate::gfx_api::GfxApi;
use crate::globals::Globals;
use crate::ifs::head_management::HeadManager;
use crate::ifs::head_management::HeadState;
use crate::ifs::jay_screencast::perform_screencast_realloc;
use crate::ifs::jay_screencast::perform_toplevel_screencasts;
use crate::ifs::wl_output::BlendSpace;
use crate::ifs::wl_output::OutputId;
use crate::ifs::wl_output::PersistentOutputState;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::handle_position_hint_requests;
use crate::ifs::wl_seat::handle_warp_mouse_to_focus;
use crate::ifs::wl_surface::NoneSurfaceExt;
use crate::ifs::wl_surface::prime::no_client_prime;
use crate::ifs::wl_surface::zwp_input_popup_surface_v2::input_popup_positioning;
use crate::ifs::wlr_output_manager::wlr_output_manager_done;
use crate::ifs::workspace_manager::workspace_manager_done;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
#[cfg(feature = "it")]
use crate::it::test_backend::TestBackend;
use crate::kbvm::KbvmContext;
use crate::leaks;
use crate::logger::Logger;
use crate::output_schedule::OutputSchedule;
use crate::portal::PortalStartup;
use crate::portal::{self};
use crate::pr_caps::PrCapsThread;
use crate::pr_caps::pr_caps;
use crate::scale::Scale;
use crate::sighand::SighandError;
use crate::sighand::{self};
use crate::sm::SessionManager;
use crate::sm::flush_toplevel_sessions;
use crate::sqlite::Sqlite;
use crate::sqlite::handle_sqlite_optimize;
use crate::state::ConnectorData;
use crate::state::IdleState;
use crate::state::ScreenlockState;
use crate::state::State;
use crate::state::TreeState;
use crate::state::XWaylandState;
use crate::syncobj::wait_for_syncobj::WaitForSyncobj;
use crate::tasks::handle_const_40hz_latch;
use crate::tasks::idle;
use crate::tasks::{self};
use crate::tracy::enable_profiler;
use crate::transactions::TransactionData;
use crate::transactions::handle_transactions_apply;
use crate::transactions::handle_transactions_timeout;
use crate::tree::DisplayNode;
use crate::tree::NodeIds;
use crate::tree::OutputNode;
use crate::tree::TearingMode;
use crate::tree::Transform;
use crate::tree::VrrMode;
use crate::tree::WorkspaceDisplayOrder;
use crate::tree::container_layout;
use crate::tree::container_render_positions;
use crate::tree::container_render_titles;
use crate::tree::float_layout;
use crate::tree::float_titles;
use crate::tree::output_render_data;
use crate::tree::placeholder_render_textures;
use crate::tree_serial_groups::handle_tree_serial_groups_scheduled;
use crate::udmabuf::UdmabufHolder;
use crate::user_session::import_environment;
use crate::user_session::start_graphical_session;
use crate::utils::bhash::BHashSet;
use crate::utils::clone3::ensure_reaper;
use crate::utils::clonecell::CloneCell;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::event_listener::handle_lazy_event_sources;
use crate::utils::fdcloser::FdCloser;
use crate::utils::nice::did_elevate_scheduler;
use crate::utils::nice::elevate_scheduler;
use crate::utils::numcell::NumCell;
use crate::utils::object_drop_queue::ObjectDropQueue;
use crate::utils::oserror::OsError;
use crate::utils::oserror::OsErrorExt;
use crate::utils::queue::AsyncQueue;
use crate::utils::rc_eq::RcEq;
use crate::utils::refcounted::RefCounted;
use crate::utils::run_toplevel::RunToplevel;
use crate::utils::sleeper::Sleeper;
use crate::utils::sleeper::start_sleeper;
use crate::utils::static_text::StaticText;
use crate::utils::tri::Try;
use crate::version::VERSION;
use crate::wheel::Wheel;
use crate::wheel::WheelError;
use clap::ValueEnum;
use forker::ForkerProxy;
use jay_config::_private::DEFAULT_SEAT_NAME;
use jay_config::logging::LogLevel as ConfigLogLevel;
use linearize::Linearize;
use log::LevelFilter;
use std::cell::Cell;
use std::cell::RefCell;
use std::env;
use std::future::Future;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use uapi::c;

pub const MAX_EXTENTS: i32 = (1 << 22) - 1;

pub const MIN_SCALE: Scale = Scale::from_wl(60);
pub const MAX_SCALE: Scale = Scale::from_int(16);

pub fn start_compositor(global: GlobalArgs, args: RunArgs) {
    sighand::reset_all();
    let reaper_pid = ensure_reaper();
    let caps = pr_caps().into_comp();
    let caps_thread = if caps.has_nice() {
        elevate_scheduler();
        Some(caps.into_thread())
    } else {
        drop(caps);
        None
    };
    let forker = create_forker(reaper_pid);
    let portal = portal::run_from_compositor(global.log_level);
    let sleeper = start_sleeper();
    enable_profiler();
    let logger = Logger::install_compositor(global.log_level);
    let portal = match portal {
        Ok(p) => Some(p),
        Err(e) => {
            log::error!("Could not spawn portal: {}", ErrorFmt(e));
            None
        }
    };
    let res = start_compositor2(
        Some(forker),
        Some(sleeper),
        portal,
        Some(logger.clone()),
        args,
        None,
        caps_thread,
    );
    leaks::log_leaked();
    if let Err(e) = res {
        let e = ErrorFmt(e);
        log::error!("A fatal error occurred: {}", e);
        eprintln!("A fatal error occurred: {}", e);
        eprintln!("See {} for more details.", logger.path());
        std::process::exit(1);
    }
    log::info!("Exit");
}

#[cfg(feature = "it")]
pub fn start_compositor_for_test(future: TestFuture) -> Result<(), CompositorError> {
    let res = start_compositor2(
        None,
        None,
        None,
        None,
        RunArgs::default(),
        Some(future),
        None,
    );
    leaks::log_leaked();
    res
}

fn create_forker(reaper_pid: c::pid_t) -> Rc<ForkerProxy> {
    match ForkerProxy::create(reaper_pid) {
        Ok(f) => Rc::new(f),
        Err(e) => fatal!("Could not create a forker process: {}", ErrorFmt(e)),
    }
}

#[derive(Debug, Error)]
pub enum CompositorError {
    #[error("The client acceptor caused an error")]
    AcceptorError(#[from] AcceptorError),
    #[error("The signal handler caused an error")]
    SighandError(#[from] SighandError),
    #[error("The clientmem subsystem caused an error")]
    ClientmemError(#[from] ClientMemError),
    #[error("The timer subsystem caused an error")]
    WheelError(#[from] WheelError),
    #[error("Could not create an io-uring")]
    IoUringError(#[from] IoUringError),
    #[error("Could not create cpu worker")]
    CpuWorkerError(#[from] CpuWorkerError),
}

pub const WAYLAND_DISPLAY: &str = "WAYLAND_DISPLAY";
pub const LIBEI_SOCKET: &str = "LIBEI_SOCKET";
pub const DISPLAY: &str = "DISPLAY";

const STATIC_VARS: &[(&str, &str)] = &[
    ("XDG_CURRENT_DESKTOP", "jay"),
    ("XDG_SESSION_TYPE", "wayland"),
    ("_JAVA_AWT_WM_NONREPARENTING", "1"),
];

pub type TestFuture = Box<dyn Fn(&Rc<State>) -> Box<dyn Future<Output = ()>>>;

fn start_compositor2(
    forker: Option<Rc<ForkerProxy>>,
    sleeper: Option<Sleeper>,
    portal: Option<PortalStartup>,
    logger: Option<Arc<Logger>>,
    run_args: RunArgs,
    test_future: Option<TestFuture>,
    caps_thread: Option<PrCapsThread>,
) -> Result<(), CompositorError> {
    let pid = uapi::getpid();
    log::info!("pid = {pid}");
    log::info!("version = {VERSION}");
    if did_elevate_scheduler() {
        log::info!("Running with elevated scheduler: SCHED_RR");
    }
    init_fd_limit();
    leaks::init();
    clientmem::init()?;
    let kb_ctx = KbvmContext::default();
    let kb_keymap = kb_ctx
        .parse_keymap(include_str!("keymap.xkb").as_bytes(), None)
        .unwrap();
    let engine = AsyncEngine::new();
    let ring = IoUring::new(&engine, 32)?;
    let _signal_future = sighand::install(&engine, &ring)?;
    let wheel = Wheel::new(&engine, &ring)?;
    let (_run_toplevel_future, run_toplevel) = RunToplevel::install(&engine);
    let node_ids = NodeIds::default();
    let scales = RefCounted::default();
    scales.add(Scale::from_int(1));
    let cpu_worker = Rc::new(CpuWorker::new(&ring, &engine)?);
    let color_manager = ColorManager::new();
    let crit_ids = Rc::new(CritMatcherIds::default());
    let eventfd_cache = EventfdCache::new(&ring, &engine);
    let sqlite = Sqlite::open(&ring, &engine, test_future.is_some())
        .inspect_err(|e| {
            log::warn!("Could not open sqlite: {}", ErrorFmt(e));
        })
        .ok();
    let sm = sqlite.as_ref().map(SessionManager::new).map(Rc::new);
    let udmabuf = Rc::new(UdmabufHolder::default());
    let no_client_prime = no_client_prime(&udmabuf);
    let tree = Rc::new(TreeState::default());
    let state = Rc::new(State {
        pid,
        kb_ctx,
        backend: CloneCell::new(Rc::new(DummyBackend)),
        forker: Default::default(),
        default_keymap: kb_keymap,
        eng: engine.clone(),
        render_ctx: Default::default(),
        render_ctx_drm_device: Default::default(),
        render_ctx_prime_copy_device: Default::default(),
        render_ctx_prime_modifiers: Default::default(),
        render_ctx_prime_modifiers_stash: Default::default(),
        render_ctx_version: NumCell::new(1),
        render_ctx_ever_initialized: Cell::new(false),
        cursors: Default::default(),
        wheel,
        clients: Clients::new(),
        globals: Globals::new(),
        connector_ids: Default::default(),
        root: Rc::new(DisplayNode::new(&tree, node_ids.next())),
        workspaces: Default::default(),
        dummy_output_id: node_ids.next(),
        dummy_output: Default::default(),
        node_ids,
        backend_events: AsyncQueue::default(),
        seat_ids: Default::default(),
        seat_queue: Default::default(),
        slow_clients: AsyncQueue::default(),
        none_surface_ext: Rc::new(NoneSurfaceExt),
        tree_changed_sent: Cell::new(false),
        config: Default::default(),
        config_locked_shortcuts: Default::default(),
        input_device_ids: Default::default(),
        input_device_handlers: Default::default(),
        theme: Default::default(),
        pending_container_layout: Default::default(),
        pending_container_render_positions: Default::default(),
        pending_container_render_title: Default::default(),
        pending_output_render_data: Default::default(),
        pending_float_layout: Default::default(),
        pending_float_titles: Default::default(),
        pending_input_popup_positioning: Default::default(),
        pending_toplevel_screencasts: Default::default(),
        pending_screencast_reallocs_or_reconfigures: Default::default(),
        pending_placeholder_render_textures: Default::default(),
        dbus: Dbus::new(&engine, &ring, &run_toplevel),
        fdcloser: FdCloser::new(),
        logger: logger.clone(),
        connectors: Default::default(),
        outputs: Default::default(),
        wlr_output_managers: Default::default(),
        drm_devs: Default::default(),
        drm_devs_by_dev_t: Default::default(),
        status: Default::default(),
        idle: IdleState {
            input: Default::default(),
            change: Default::default(),
            timeout: Cell::new(Duration::from_secs(10 * 60)),
            grace_period: Cell::new(Duration::from_secs(5)),
            timeout_changed: Default::default(),
            inhibitors: Default::default(),
            inhibitors_changed: Default::default(),
            inhibited_idle_notifications: Default::default(),
            backend_idle: Cell::new(true),
            in_grace_period: Cell::new(false),
        },
        run_args,
        xwayland: XWaylandState {
            enabled: Cell::new(true),
            running: Cell::new(false),
            pidfd: Default::default(),
            handler: Default::default(),
            queue: Default::default(),
            ipc_device_ids: Default::default(),
            use_wire_scale: Default::default(),
            wire_scale: Default::default(),
            windows: Default::default(),
            client: Default::default(),
            display: Default::default(),
        },
        acceptor: Default::default(),
        serial: Default::default(),
        idle_inhibitor_ids: Default::default(),
        run_toplevel,
        config_dir: config_dir(),
        tracker: Default::default(),
        data_offer_ids: Default::default(),
        data_source_ids: Default::default(),
        drm_dev_ids: Default::default(),
        ring: ring.clone(),
        lock: ScreenlockState {
            locked: Default::default(),
            lock: Default::default(),
        },
        scales,
        cursor_sizes: Default::default(),
        hardware_tick_cursor: Default::default(),
        testers: Default::default(),
        render_ctx_watchers: Default::default(),
        workspace_watchers: Default::default(),
        default_workspace_capture: Cell::new(true),
        default_gfx_api: Cell::new(GfxApi::Vulkan),
        activation_tokens: Default::default(),
        toplevel_lists: Default::default(),
        dma_buf_ids: Default::default(),
        direct_scanout_enabled: Cell::new(true),
        persistent_output_states: Default::default(),
        double_click_interval_usec: Cell::new(400 * 1000),
        double_click_distance: Cell::new(5),
        create_default_seat: Cell::new(true),
        subsurface_ids: Default::default(),
        wait_for_syncobj: Rc::new(WaitForSyncobj::new(&ring, &engine)),
        explicit_sync_enabled: Cell::new(true),
        explicit_sync_supported: Default::default(),
        keyboard_state_ids: Default::default(),
        physical_keyboard_ids: Default::default(),
        security_context_acceptors: Default::default(),
        tagged_acceptors: Default::default(),
        cursor_user_group_ids: Default::default(),
        cursor_user_ids: Default::default(),
        cursor_user_groups: Default::default(),
        cursor_user_group_hardware_cursor: Default::default(),
        input_device_group_ids: Default::default(),
        tablet_ids: Default::default(),
        tablet_tool_ids: Default::default(),
        tablet_pad_ids: Default::default(),
        damage_visualizer: DamageVisualizer::new(&engine, &color_manager),
        default_vrr_mode: Cell::new(*VrrMode::NEVER),
        default_vrr_cursor_hz: Cell::new(None),
        default_tearing_mode: Cell::new(*TearingMode::VARIANT_3),
        ei_acceptor: Default::default(),
        ei_acceptor_future: Default::default(),
        enable_ei_acceptor: Default::default(),
        ei_clients: EiClients::new(),
        slow_ei_clients: Default::default(),
        cpu_worker,
        ui_drag_enabled: Cell::new(true),
        ui_drag_threshold_squared: Cell::new(10),
        toplevels: Default::default(),
        const_40hz_latch: Default::default(),
        tray_item_ids: Default::default(),
        data_control_device_ids: Default::default(),
        workspace_managers: Default::default(),
        color_management_enabled: Cell::new(false),
        color_manager,
        float_above_fullscreen: Cell::new(false),
        icons: Default::default(),
        show_pin_icon: Cell::new(false),
        cl_matcher_manager: ClMatcherManager::new(&crit_ids),
        tl_matcher_manager: TlMatcherManager::new(&crit_ids),
        caps_thread,
        toplevel_managers: Default::default(),
        node_at_tree: Default::default(),
        position_hint_requests: Default::default(),
        pending_warp_mouse_to_focus: Default::default(),
        backend_connector_state_serials: Default::default(),
        head_names: Default::default(),
        show_bar: Cell::new(true),
        enable_primary_selection: Cell::new(true),
        workspace_display_order: Cell::new(WorkspaceDisplayOrder::Manual),
        outputs_without_hc: Default::default(),
        udmabuf,
        gfx_ctx_changed: Default::default(),
        copy_device_registry: Rc::new(CopyDeviceRegistry::new(&ring, &engine, &eventfd_cache)),
        buffer_id_device_registry: Default::default(),
        supports_presentation_feedback: Default::default(),
        eventfd_cache,
        lazy_event_sources: Default::default(),
        bo_drop_queue: Rc::new(ObjectDropQueue::new(&ring)),
        egg_state: Default::default(),
        control_centers: Default::default(),
        virtual_outputs: Default::default(),
        clean_logs_older_than: Default::default(),
        sqlite,
        sm,
        session_management_enabled: Cell::new(true),
        fallback_output: Default::default(),
        toplevel_icon_ids: Default::default(),
        toplevel_icons: Default::default(),
        transaction_data: TransactionData::new(&tree),
        tree,
        commit_cache: Default::default(),
        dmabuf_feedback: Default::default(),
        surface_pending_cache: Default::default(),
        no_client_prime,
        lazy_prime_buffer_resv_user: Default::default(),
        visualize_compositing: Default::default(),
        sleeper,
    });
    state.tracker.register(ClientId::from_raw(0));
    create_dummy_output(&state);
    let (acceptor, _acceptor_future) = Acceptor::install(&state)?;
    if let Some(forker) = forker {
        forker.install(&state);
        forker.setenv(
            WAYLAND_DISPLAY.as_bytes(),
            acceptor.socket_name().as_bytes(),
        );
        for (key, val) in STATIC_VARS {
            forker.setenv(key.as_bytes(), val.as_bytes());
        }
    }
    let mut _portal = None;
    if let (Some(portal), Some(logger)) = (portal, &logger) {
        _portal = Some(engine.spawn(
            "portal",
            portal.spawn(engine.clone(), ring.clone(), logger.clone()),
        ));
    }
    let compositor = engine.spawn("compositor", start_compositor3(state.clone(), test_future));
    ring.run()?;
    drop(compositor);
    state.clear();
    engine.clear();
    Ok(())
}

async fn start_compositor3(state: Rc<State>, test_future: Option<TestFuture>) {
    let is_test = test_future.is_some();

    let backend = match create_backend(&state, test_future).await {
        Some(b) => b,
        _ => {
            log::error!("Could not create a backend");
            state.ring.stop();
            return;
        }
    };
    state.backend.set(backend.clone());
    state
        .supports_presentation_feedback
        .set(backend.supports_presentation_feedback());

    if backend.import_environment() {
        if let Some(acc) = state.acceptor.get() {
            import_environment(&state, WAYLAND_DISPLAY, acc.socket_name()).await;
        }
        for (key, val) in STATIC_VARS {
            import_environment(&state, key, val).await;
        }
    }

    start_graphical_session(&state).await;

    let config = load_config(&state, is_test);
    config.configure(false);
    state.config.set(Some(Rc::new(config)));

    if state.create_default_seat.get() && state.globals.seats.is_empty() {
        state.create_seat(DEFAULT_SEAT_NAME);
    }
    state.perform_clean_logs_older_than();
    state.update_ei_acceptor();

    if is_test {
        state.set_configure_timeout_ns(0);
        state.set_transaction_timeout_ns(0);
    }

    let _geh = start_global_event_handlers(&state);
    state.start_xwayland();

    match backend.run().await {
        Err(e) => log::error!("Backend failed: {}", ErrorFmt(e.deref())),
        _ => log::error!("Backend stopped without an error"),
    }
    state.ring.stop();
}

fn load_config(
    state: &Rc<State>,
    #[allow(clippy::allow_attributes, unused_variables)] for_test: bool,
) -> ConfigProxy {
    #[cfg(feature = "it")]
    if for_test {
        return ConfigProxy::for_test(state);
    }
    match ConfigProxy::from_config_dir(state) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Could not load config.so: {}", ErrorFmt(e));
            log::warn!("Using default config");
            ConfigProxy::default(state)
        }
    }
}

fn start_global_event_handlers(state: &Rc<State>) -> Vec<SpawnedFuture<()>> {
    let eng = &state.eng;

    vec![
        eng.spawn(
            "backend events",
            tasks::handle_backend_events(state.clone()),
        ),
        eng.spawn("slow client", tasks::handle_slow_clients(state.clone())),
        eng.spawn(
            "handware cursor tick",
            tasks::handle_hardware_cursor_tick(state.clone()),
        ),
        eng.spawn2(
            "container layout",
            Phase::Layout,
            container_layout(state.clone()),
        ),
        eng.spawn2(
            "container render positions",
            Phase::PostLayout,
            container_render_positions(state.clone()),
        ),
        eng.spawn2(
            "container titles",
            Phase::PostLayout,
            container_render_titles(state.clone()),
        ),
        eng.spawn2(
            "placeholder textures",
            Phase::PostLayout,
            placeholder_render_textures(state.clone()),
        ),
        eng.spawn2(
            "output render",
            Phase::PostLayout,
            output_render_data(state.clone()),
        ),
        eng.spawn(
            "wlr output manager done",
            wlr_output_manager_done(state.clone()),
        ),
        eng.spawn2("float layout", Phase::Layout, float_layout(state.clone())),
        eng.spawn2(
            "float titles",
            Phase::PostLayout,
            float_titles(state.clone()),
        ),
        eng.spawn2("idle", Phase::PostLayout, idle(state.clone())),
        eng.spawn2(
            "input, popup positioning",
            Phase::PostLayout,
            input_popup_positioning(state.clone()),
        ),
        eng.spawn2(
            "toplevel screencast present",
            Phase::Present,
            perform_toplevel_screencasts(state.clone()),
        ),
        eng.spawn2(
            "screencast realloc",
            Phase::PostLayout,
            perform_screencast_realloc(state.clone()),
        ),
        eng.spawn2(
            "visualize damage",
            Phase::PostLayout,
            visualize_damage(state.clone()),
        ),
        eng.spawn(
            "slow ei clients",
            tasks::handle_slow_ei_clients(state.clone()),
        ),
        eng.spawn2(
            "const 40hz latch",
            Phase::Present,
            handle_const_40hz_latch(state.clone()),
        ),
        eng.spawn(
            "workspace manager done",
            workspace_manager_done(state.clone()),
        ),
        eng.spawn("cl matcher manager", handle_cl_changes(state.clone())),
        eng.spawn(
            "cl matcher leaf events",
            handle_cl_leaf_events(state.clone()),
        ),
        eng.spawn("tl matcher manager", handle_tl_changes(state.clone())),
        eng.spawn(
            "tl matcher leaf events",
            handle_tl_leaf_events(state.clone()),
        ),
        eng.spawn2(
            "tl matcher just mapped",
            Phase::Layout,
            handle_tl_just_mapped(state.clone()),
        ),
        eng.spawn(
            "position hint requests",
            handle_position_hint_requests(state.clone()),
        ),
        eng.spawn(
            "lazy event sources",
            handle_lazy_event_sources(state.clone()),
        ),
        eng.spawn(
            "redraw control centers",
            redraw_control_centers(state.clone()),
        ),
        eng.spawn(
            "warp mouse to focus",
            handle_warp_mouse_to_focus(state.clone()),
        ),
        eng.spawn("optimize sqlite", handle_sqlite_optimize(state.clone())),
        eng.spawn(
            "flush toplevel sessions",
            flush_toplevel_sessions(state.clone()),
        ),
        eng.spawn2(
            "tree serial groups scheduled",
            Phase::PostLayout,
            handle_tree_serial_groups_scheduled(state.clone()),
        ),
        eng.spawn2(
            "configurables apply",
            Phase::PostLayout,
            handle_configurables_apply(state.clone()),
        ),
        eng.spawn2(
            "configurables timeout",
            Phase::PostLayout,
            handle_configurables_timeout(state.clone()),
        ),
        eng.spawn(
            "dmabuf feedback changes",
            handle_dmabuf_feedback_changes(state.clone()),
        ),
        eng.spawn(
            "transactions apply",
            handle_transactions_apply(state.clone()),
        ),
        eng.spawn(
            "transactions timeout",
            handle_transactions_timeout(state.clone()),
        ),
    ]
}

async fn create_backend(
    state: &Rc<State>,
    #[allow(clippy::allow_attributes, unused_variables)] test_future: Option<TestFuture>,
) -> Option<Rc<dyn Backend>> {
    #[cfg(feature = "it")]
    if let Some(tf) = test_future {
        return Some(Rc::new(TestBackend::new(state, tf)));
    }
    let mut backends = &state.run_args.backends[..];
    if backends.is_empty() {
        backends = &[CliBackend::X11, CliBackend::Metal];
    }
    let mut tried_backends = BHashSet::default();
    for &backend in backends {
        if !tried_backends.insert(backend) {
            continue;
        }
        match backend {
            CliBackend::X11 => {
                log::info!("Trying to create X backend");
                match x::create(state).await {
                    Ok(b) => return Some(b),
                    Err(e) => {
                        log::error!("Could not create X backend: {}", ErrorFmt(e));
                    }
                }
            }
            CliBackend::Metal => {
                log::info!("Trying to create metal backend");
                match metal::create(state).await {
                    Ok(b) => return Some(b),
                    Err(e) => {
                        log::error!("Could not create metal backend: {}", ErrorFmt(e));
                    }
                }
            }
            CliBackend::Headless => {
                log::info!("Trying to create headless backend");
                match headless::create(state).await {
                    Ok(b) => return Some(b),
                    Err(e) => {
                        log::error!("Could not create headless backend: {}", ErrorFmt(e));
                    }
                }
            }
        }
    }
    None
}

fn init_fd_limit() {
    let res = OsError::tri(|| {
        let mut cur = uapi::getrlimit(c::RLIMIT_NOFILE as _).to_os_error()?;
        if cur.rlim_cur < cur.rlim_max {
            log::info!(
                "Increasing file descriptor limit from {} to {}",
                cur.rlim_cur,
                cur.rlim_max
            );
            cur.rlim_cur = cur.rlim_max;
            uapi::setrlimit(c::RLIMIT_NOFILE as _, &cur).to_os_error()?;
        }
        Ok(())
    });
    if let Err(e) = res {
        log::warn!("Could not increase file descriptor limit: {}", ErrorFmt(e));
    }
}

fn create_dummy_output(state: &Rc<State>) {
    let output_id = OutputId::new("jay-dummy-connector", "jay", "jay-dummy-output", "");
    let persistent_state = Rc::new(PersistentOutputState::default());
    let id = state.connector_ids.next();
    let connector = Rc::new(DummyOutput { id }) as Rc<dyn Connector>;
    let backend_state = connector.state();
    let name = Rc::new("Dummy".to_string());
    let head_name = state.head_names.next();
    let head_state = HeadState {
        connector_id: id,
        name: RcEq(name.clone()),
        position: (0, 0),
        size: (0, 0),
        active: false,
        connected: false,
        transform: Transform::None,
        scale: Default::default(),
        scaling_filter: Default::default(),
        wl_output: None,
        connector_enabled: true,
        in_compositor_space: false,
        mode: Default::default(),
        monitor_info: None,
        inherent_non_desktop: false,
        override_non_desktop: None,
        vrr: false,
        vrr_mode: VrrMode::Never,
        tearing_enabled: backend_state.tearing,
        tearing_active: false,
        tearing_mode: TearingMode::Never,
        format: XRGB8888,
        color_space: backend_state.color_space,
        eotf: backend_state.eotf,
        supported_formats: Default::default(),
        brightness: None,
        blend_space: BlendSpace::Srgb,
        use_native_gamut: false,
        vrr_cursor_hz: None,
        persistent_state: Some(RcEq(persistent_state.clone())),
    };
    let connector_data = Rc::new(ConnectorData {
        id,
        connector,
        handler: Cell::new(None),
        connected: Cell::new(true),
        name,
        description: Default::default(),
        drm_dev: None,
        async_event: Default::default(),
        damaged: Cell::new(false),
        damage: Default::default(),
        needs_vblank_emulation: Cell::new(false),
        damage_intersect: Default::default(),
        state: RefCell::new(backend_state),
        head_manager: HeadManager::new(head_name, head_state),
        wlr_output_heads: Default::default(),
    });
    let schedule = Rc::new(OutputSchedule::new(
        state,
        &connector_data,
        &persistent_state,
    ));
    let global = Rc::new(WlOutputGlobal::new(
        state.globals.name(),
        state,
        &connector_data,
        Some(Vec::new()),
        0,
        0,
        &output_id,
        &persistent_state,
        Vec::new(),
        Vec::new(),
        Primaries::SRGB,
        None,
    ));
    let dummy_output = OutputNode::new(state.dummy_output_id, &global, &schedule);
    state.dummy_output.set(Some(dummy_output));
}

pub fn config_dir() -> Option<String> {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        Some(format!("{}/jay", xdg))
    } else if let Ok(home) = env::var("HOME") {
        Some(format!("{}/.config/jay", home))
    } else {
        log::warn!("Neither XDG_CONFIG_HOME nor HOME are set. Using default config.");
        None
    }
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash, Default, Eq, PartialEq, Linearize)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Off,
}

impl Into<LevelFilter> for LogLevel {
    fn into(self) -> LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::Trace,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Off => LevelFilter::Off,
        }
    }
}

impl From<LevelFilter> for LogLevel {
    fn from(value: LevelFilter) -> Self {
        match value {
            LevelFilter::Trace => LogLevel::Trace,
            LevelFilter::Debug => LogLevel::Debug,
            LevelFilter::Info => LogLevel::Info,
            LevelFilter::Warn => LogLevel::Warn,
            LevelFilter::Error => LogLevel::Error,
            LevelFilter::Off => LogLevel::Off,
        }
    }
}

impl StaticText for LogLevel {
    fn text(&self) -> &'static str {
        match self {
            LogLevel::Off => "Off",
            LogLevel::Error => "Error",
            LogLevel::Warn => "Warn",
            LogLevel::Info => "Info",
            LogLevel::Debug => "Debug",
            LogLevel::Trace => "Trace",
        }
    }
}

impl From<ConfigLogLevel> for LogLevel {
    fn from(value: ConfigLogLevel) -> Self {
        match value {
            ConfigLogLevel::Trace => LogLevel::Trace,
            ConfigLogLevel::Debug => LogLevel::Debug,
            ConfigLogLevel::Info => LogLevel::Info,
            ConfigLogLevel::Warn => LogLevel::Warn,
            ConfigLogLevel::Error => LogLevel::Error,
        }
    }
}

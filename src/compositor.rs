#[cfg(feature = "it")]
use crate::it::test_backend::TestBackend;
use {
    crate::{
        acceptor::{Acceptor, AcceptorError},
        async_engine::{AsyncEngine, Phase, SpawnedFuture},
        backend::{self, Backend, BackendConnectorState, BackendConnectorStateSerial, Connector},
        backends::{
            dummy::{DummyBackend, DummyOutput},
            metal, x,
        },
        cli::{CliBackend, GlobalArgs, RunArgs},
        client::{ClientId, Clients},
        clientmem::{self, ClientMemError},
        cmm::{cmm_manager::ColorManager, cmm_primaries::Primaries},
        config::ConfigProxy,
        configurable::{handle_configurables, handle_configurables_timeout},
        cpu_worker::{CpuWorker, CpuWorkerError},
        criteria::{
            CritMatcherIds,
            clm::{ClMatcherManager, handle_cl_changes, handle_cl_leaf_events},
            tlm::{
                TlMatcherManager, handle_tl_changes, handle_tl_just_mapped, handle_tl_leaf_events,
            },
        },
        damage::{DamageVisualizer, visualize_damage},
        dbus::Dbus,
        ei::ei_client::EiClients,
        forker,
        format::XRGB8888,
        globals::Globals,
        ifs::{
            head_management::{
                HeadManagers, HeadState, jay_head_manager_session_v1::handle_jay_head_manager_done,
            },
            jay_screencast::{perform_screencast_realloc, perform_toplevel_screencasts},
            wl_output::{BlendSpace, OutputId, PersistentOutputState, WlOutputGlobal},
            wl_seat::handle_position_hint_requests,
            wl_surface::{NoneSurfaceExt, zwp_input_popup_surface_v2::input_popup_positioning},
            wlr_output_manager::wlr_output_manager_done,
            workspace_manager::workspace_manager_done,
        },
        io_uring::{IoUring, IoUringError},
        kbvm::KbvmContext,
        leaks,
        logger::Logger,
        output_schedule::OutputSchedule,
        portal::{self, PortalStartup},
        pr_caps::{PrCapsThread, pr_caps},
        scale::Scale,
        sighand::{self, SighandError},
        state::{ConnectorData, IdleState, ScreenlockState, State, XWaylandState},
        tasks::{self, handle_const_40hz_latch, idle},
        tracy::enable_profiler,
        tree::{
            DisplayNode, NodeIds, OutputNode, TearingMode, VrrMode, WorkspaceInOutput,
            WorkspaceNode, WorkspaceState, container_layout, container_render_positions,
            container_render_titles, float_layout, float_titles, output_render_data,
            placeholder_render_textures,
            transaction::{handle_tree_blocker_timeout, handle_tree_blocker_unblocked},
        },
        user_session::import_environment,
        utils::{
            clone3::ensure_reaper,
            clonecell::CloneCell,
            errorfmt::ErrorFmt,
            fdcloser::FdCloser,
            nice::{did_elevate_scheduler, elevate_scheduler},
            numcell::NumCell,
            oserror::OsError,
            queue::AsyncQueue,
            rc_eq::RcEq,
            refcounted::RefCounted,
            run_toplevel::RunToplevel,
            tri::Try,
        },
        version::VERSION,
        video::drm::wait_for_sync_obj::WaitForSyncObj,
        wheel::{Wheel, WheelError},
    },
    ahash::AHashSet,
    forker::ForkerProxy,
    jay_config::{
        _private::DEFAULT_SEAT_NAME,
        video::{GfxApi, Transform},
        workspace::WorkspaceDisplayOrder,
    },
    std::{cell::Cell, env, future::Future, ops::Deref, rc::Rc, sync::Arc, time::Duration},
    thiserror::Error,
    uapi::c,
};

pub const MAX_EXTENTS: i32 = (1 << 22) - 1;

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
    let portal = portal::run_from_compositor(global.log_level.into());
    enable_profiler();
    let logger = Logger::install_compositor(global.log_level.into());
    let portal = match portal {
        Ok(p) => Some(p),
        Err(e) => {
            log::error!("Could not spawn portal: {}", ErrorFmt(e));
            None
        }
    };
    let res = start_compositor2(
        Some(forker),
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
    let res = start_compositor2(None, None, None, RunArgs::default(), Some(future), None);
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
    portal: Option<PortalStartup>,
    logger: Option<Arc<Logger>>,
    run_args: RunArgs,
    test_future: Option<TestFuture>,
    caps_thread: Option<PrCapsThread>,
) -> Result<(), CompositorError> {
    log::info!("pid = {}", uapi::getpid());
    log::info!("version = {VERSION}");
    if did_elevate_scheduler() {
        log::info!("Running with elevated scheduler: SCHED_RR");
    }
    init_fd_limit();
    leaks::init();
    clientmem::init()?;
    let kb_ctx = KbvmContext::default();
    let kb_keymap = kb_ctx
        .parse_keymap(include_str!("keymap.xkb").as_bytes())
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
    let state = Rc::new(State {
        kb_ctx,
        backend: CloneCell::new(Rc::new(DummyBackend)),
        forker: Default::default(),
        default_keymap: kb_keymap,
        eng: engine.clone(),
        render_ctx: Default::default(),
        drm_feedback: Default::default(),
        drm_feedback_consumers: Default::default(),
        render_ctx_version: NumCell::new(1),
        render_ctx_ever_initialized: Cell::new(false),
        cursors: Default::default(),
        wheel,
        clients: Clients::new(),
        globals: Globals::new(),
        connector_ids: Default::default(),
        root: Rc::new(DisplayNode::new(node_ids.next())),
        workspaces: Default::default(),
        dummy_output: Default::default(),
        node_ids,
        backend_events: AsyncQueue::new(),
        seat_ids: Default::default(),
        seat_queue: Default::default(),
        slow_clients: AsyncQueue::new(),
        none_surface_ext: Rc::new(NoneSurfaceExt),
        tree_changed_sent: Cell::new(false),
        config: Default::default(),
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
            pidfd: Default::default(),
            handler: Default::default(),
            queue: Default::default(),
            ipc_device_ids: Default::default(),
            use_wire_scale: Default::default(),
            wire_scale: Default::default(),
            windows: Default::default(),
        },
        acceptor: Default::default(),
        serial: Default::default(),
        idle_inhibitor_ids: Default::default(),
        run_toplevel,
        config_dir: config_dir(),
        config_file_id: NumCell::new(1),
        tracker: Default::default(),
        data_offer_ids: Default::default(),
        data_source_ids: Default::default(),
        drm_dev_ids: Default::default(),
        ring: ring.clone(),
        lock: ScreenlockState {
            locked: Cell::new(false),
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
        drm_feedback_ids: Default::default(),
        direct_scanout_enabled: Cell::new(true),
        persistent_output_states: Default::default(),
        double_click_interval_usec: Cell::new(400 * 1000),
        double_click_distance: Cell::new(5),
        create_default_seat: Cell::new(true),
        subsurface_ids: Default::default(),
        wait_for_sync_obj: Rc::new(WaitForSyncObj::new(&ring, &engine)),
        explicit_sync_enabled: Cell::new(true),
        keyboard_state_ids: Default::default(),
        physical_keyboard_ids: Default::default(),
        security_context_acceptors: Default::default(),
        cursor_user_group_ids: Default::default(),
        cursor_user_ids: Default::default(),
        cursor_user_groups: Default::default(),
        cursor_user_group_hardware_cursor: Default::default(),
        input_device_group_ids: Default::default(),
        tablet_ids: Default::default(),
        tablet_tool_ids: Default::default(),
        tablet_pad_ids: Default::default(),
        damage_visualizer: DamageVisualizer::new(&engine, &color_manager),
        default_vrr_mode: Cell::new(VrrMode::NEVER),
        default_vrr_cursor_hz: Cell::new(None),
        default_tearing_mode: Cell::new(TearingMode::VARIANT_3),
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
        backend_connector_state_serials: Default::default(),
        head_names: Default::default(),
        head_managers: Default::default(),
        head_managers_async: Default::default(),
        show_bar: Cell::new(true),
        enable_primary_selection: Cell::new(true),
        workspace_display_order: Cell::new(WorkspaceDisplayOrder::Manual),
        outputs_without_hc: Default::default(),
        udmabuf: Default::default(),
        tree_serials: Default::default(),
        configure_groups: Default::default(),
        tree_transactions: Default::default(),
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
    let _compositor = engine.spawn("compositor", start_compositor3(state.clone(), test_future));
    ring.run()?;
    state.clear();
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
    state.globals.add_backend_singletons(&backend);

    if backend.import_environment() {
        if let Some(acc) = state.acceptor.get() {
            import_environment(&state, WAYLAND_DISPLAY, acc.socket_name()).await;
        }
        for (key, val) in STATIC_VARS {
            import_environment(&state, key, val).await;
        }
    }

    let config = load_config(&state, is_test);
    config.configure(false);
    state.config.set(Some(Rc::new(config)));

    if state.create_default_seat.get() && state.globals.seats.is_empty() {
        state.create_seat(DEFAULT_SEAT_NAME);
    }
    state.update_ei_acceptor();

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
        eng.spawn2(
            "jay head manager send done",
            Phase::Layout,
            handle_jay_head_manager_done(state.clone()),
        ),
        eng.spawn2(
            "configurables",
            Phase::PostLayout,
            handle_configurables(state.clone()),
        ),
        eng.spawn2(
            "configurables timeout",
            Phase::PostLayout,
            handle_configurables_timeout(state.clone()),
        ),
        eng.spawn(
            "tree blocker unblocked",
            handle_tree_blocker_unblocked(state.clone()),
        ),
        eng.spawn(
            "tree blocker timeout",
            handle_tree_blocker_timeout(state.clone()),
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
    let mut tried_backends = AHashSet::new();
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
        }
    }
    None
}

fn init_fd_limit() {
    let res = OsError::tri(|| {
        let mut cur = uapi::getrlimit(c::RLIMIT_NOFILE as _)?;
        if cur.rlim_cur < cur.rlim_max {
            log::info!(
                "Increasing file descriptor limit from {} to {}",
                cur.rlim_cur,
                cur.rlim_max
            );
            cur.rlim_cur = cur.rlim_max;
            uapi::setrlimit(c::RLIMIT_NOFILE as _, &cur)?;
        }
        Ok(())
    });
    if let Err(e) = res {
        log::warn!("Could not increase file descriptor limit: {}", ErrorFmt(e));
    }
}

fn create_dummy_output(state: &Rc<State>) {
    let output_id = Rc::new(OutputId {
        connector: Some("jay-dummy-connector".to_string()),
        manufacturer: "jay".to_string(),
        model: "jay-dummy-output".to_string(),
        serial_number: "".to_string(),
    });
    let persistent_state = Rc::new(PersistentOutputState {
        transform: Default::default(),
        scale: Default::default(),
        pos: Default::default(),
        vrr_mode: Cell::new(VrrMode::NEVER),
        vrr_cursor_hz: Default::default(),
        tearing_mode: Cell::new(&TearingMode::Never),
        brightness: Cell::new(None),
        blend_space: Cell::new(BlendSpace::Srgb),
    });
    let mode = backend::Mode {
        width: 0,
        height: 0,
        refresh_rate_millihz: 40_000,
    };
    let backend_state = BackendConnectorState {
        serial: BackendConnectorStateSerial::from_raw(0),
        enabled: true,
        active: false,
        mode,
        non_desktop_override: None,
        vrr: false,
        tearing: false,
        format: XRGB8888,
        color_space: Default::default(),
        eotf: Default::default(),
    };
    let id = state.connector_ids.next();
    let connector = Rc::new(DummyOutput { id }) as Rc<dyn Connector>;
    let name = Rc::new("Dummy".to_string());
    let head_name = state.head_names.next();
    let head_state = HeadState {
        name: RcEq(name.clone()),
        position: (0, 0),
        size: (0, 0),
        active: false,
        connected: false,
        transform: Transform::None,
        scale: Default::default(),
        wl_output: None,
        connector_enabled: true,
        in_compositor_space: false,
        mode: Default::default(),
        monitor_info: None,
        inherent_non_desktop: false,
        override_non_desktop: None,
        vrr: false,
        vrr_mode: VrrMode::Never.to_config(),
        tearing_enabled: backend_state.tearing,
        tearing_active: false,
        tearing_mode: TearingMode::Never.to_config(),
        format: XRGB8888,
        color_space: backend_state.color_space,
        eotf: backend_state.eotf,
        supported_formats: Default::default(),
        brightness: None,
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
        state: Cell::new(backend_state),
        head_managers: HeadManagers::new(head_name, head_state),
        wlr_output_heads: Default::default(),
    });
    let schedule = Rc::new(OutputSchedule::new(
        &state.ring,
        &state.eng,
        &connector_data,
        &persistent_state,
    ));
    let dummy_output = Rc::new(OutputNode {
        id: state.node_ids.next(),
        global: Rc::new(WlOutputGlobal::new(
            state.globals.name(),
            state,
            &connector_data,
            Vec::new(),
            0,
            0,
            &output_id,
            &persistent_state,
            Vec::new(),
            Vec::new(),
            Primaries::SRGB,
            None,
        )),
        jay_outputs: Default::default(),
        workspaces: Default::default(),
        workspace: Default::default(),
        seat_state: Default::default(),
        layers: Default::default(),
        exclusive_zones: Default::default(),
        workspace_rect: Default::default(),
        non_exclusive_rect_rel: Default::default(),
        non_exclusive_rect: Default::default(),
        render_data: Default::default(),
        state: state.clone(),
        is_dummy: true,
        status: Default::default(),
        scroll: Default::default(),
        pointer_positions: Default::default(),
        pointer_down: Default::default(),
        lock_surface: Default::default(),
        hardware_cursor: Default::default(),
        update_render_data_scheduled: Cell::new(false),
        screencasts: Default::default(),
        hardware_cursor_needs_render: Cell::new(false),
        screencopies: Default::default(),
        title_visible: Cell::new(false),
        schedule,
        vblank_event: Default::default(),
        latch_event: Default::default(),
        presentation_event: Default::default(),
        render_margin_ns: Default::default(),
        flip_margin_ns: Default::default(),
        ext_copy_sessions: Default::default(),
        before_latch_event: Default::default(),
        tray_start_rel: Default::default(),
        tray_items: Default::default(),
        ext_workspace_groups: Default::default(),
        pinned: Default::default(),
        tearing: Default::default(),
    });
    let dummy_workspace = Rc::new(WorkspaceNode {
        id: state.node_ids.next(),
        state: state.clone(),
        is_dummy: true,
        stacked: Default::default(),
        seat_state: Default::default(),
        name: "dummy".to_string(),
        visible_on_desired_output: Default::default(),
        desired_output: CloneCell::new(dummy_output.global.output_id.clone()),
        jay_workspaces: Default::default(),
        may_capture: Cell::new(false),
        has_capture: Cell::new(false),
        title_texture: Default::default(),
        attention_requests: Default::default(),
        render_highlight: Default::default(),
        ext_workspaces: Default::default(),
        opt: Default::default(),
        current: WorkspaceState {
            output: CloneCell::new(dummy_output.clone()),
            output_id: Cell::new(dummy_output.id),
            position: Default::default(),
            container: Default::default(),
            output_link: Default::default(),
            visible: Default::default(),
            fullscreen: Default::default(),
        },
        mapped: WorkspaceState {
            output: CloneCell::new(dummy_output.clone()),
            output_id: Cell::new(dummy_output.id),
            position: Default::default(),
            container: Default::default(),
            output_link: Default::default(),
            visible: Default::default(),
            fullscreen: Default::default(),
        },
    });
    dummy_workspace.current.output_link.set(Some(Rc::new(
        dummy_output
            .workspaces
            .add_last(WorkspaceInOutput::new(&dummy_workspace)),
    )));
    let tt = &state.tree_transaction();
    dummy_output.show_workspace(tt, &dummy_workspace);
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

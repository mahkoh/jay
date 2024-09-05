#[cfg(feature = "it")]
use crate::it::test_backend::TestBackend;
use {
    crate::{
        acceptor::{Acceptor, AcceptorError},
        async_engine::{AsyncEngine, Phase, SpawnedFuture},
        backend::{self, Backend, Connector},
        backends::{
            dummy::{DummyBackend, DummyOutput},
            metal, x,
        },
        cli::{CliBackend, GlobalArgs, RunArgs},
        client::{ClientId, Clients},
        clientmem::{self, ClientMemError},
        config::ConfigProxy,
        damage::{visualize_damage, DamageVisualizer},
        dbus::Dbus,
        ei::ei_client::EiClients,
        forker,
        globals::Globals,
        ifs::{
            jay_screencast::{perform_screencast_realloc, perform_toplevel_screencasts},
            wl_output::{OutputId, PersistentOutputState, WlOutputGlobal},
            wl_surface::{zwp_input_popup_surface_v2::input_popup_positioning, NoneSurfaceExt},
        },
        io_uring::{IoUring, IoUringError},
        leaks,
        logger::Logger,
        output_schedule::OutputSchedule,
        portal::{self, PortalStartup},
        scale::Scale,
        sighand::{self, SighandError},
        state::{ConnectorData, IdleState, ScreenlockState, State, XWaylandState},
        tasks::{self, idle},
        tree::{
            container_layout, container_render_data, float_layout, float_titles,
            output_render_data, DisplayNode, NodeIds, OutputNode, TearingMode, VrrMode,
            WorkspaceNode,
        },
        user_session::import_environment,
        utils::{
            clonecell::CloneCell, errorfmt::ErrorFmt, fdcloser::FdCloser, numcell::NumCell,
            oserror::OsError, queue::AsyncQueue, refcounted::RefCounted, run_toplevel::RunToplevel,
            tri::Try,
        },
        version::VERSION,
        video::drm::wait_for_sync_obj::WaitForSyncObj,
        wheel::{Wheel, WheelError},
        xkbcommon::XkbContext,
    },
    ahash::AHashSet,
    forker::ForkerProxy,
    jay_config::{_private::DEFAULT_SEAT_NAME, video::GfxApi},
    std::{cell::Cell, env, future::Future, ops::Deref, rc::Rc, sync::Arc, time::Duration},
    thiserror::Error,
    uapi::c,
};

pub const MAX_EXTENTS: i32 = (1 << 22) - 1;

pub fn start_compositor(global: GlobalArgs, args: RunArgs) {
    let forker = create_forker();
    let portal = portal::run_from_compositor(global.log_level.into());
    let logger = Logger::install_compositor(global.log_level.into());
    let portal = match portal {
        Ok(p) => Some(p),
        Err(e) => {
            log::error!("Could not spawn portal: {}", ErrorFmt(e));
            None
        }
    };
    let res = start_compositor2(Some(forker), portal, Some(logger.clone()), args, None);
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
    let res = start_compositor2(None, None, None, RunArgs::default(), Some(future));
    leaks::log_leaked();
    res
}

fn create_forker() -> Rc<ForkerProxy> {
    match ForkerProxy::create() {
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
) -> Result<(), CompositorError> {
    log::info!("pid = {}", uapi::getpid());
    log::info!("version = {VERSION}");
    init_fd_limit();
    leaks::init();
    clientmem::init()?;
    let xkb_ctx = XkbContext::new().unwrap();
    let xkb_keymap = xkb_ctx.keymap_from_str(include_str!("keymap.xkb")).unwrap();
    let engine = AsyncEngine::new();
    let ring = IoUring::new(&engine, 32)?;
    let _signal_future = sighand::install(&engine, &ring)?;
    let wheel = Wheel::new(&engine, &ring)?;
    let (_run_toplevel_future, run_toplevel) = RunToplevel::install(&engine);
    let node_ids = NodeIds::default();
    let scales = RefCounted::default();
    scales.add(Scale::from_int(1));
    let state = Rc::new(State {
        xkb_ctx,
        backend: CloneCell::new(Rc::new(DummyBackend)),
        forker: Default::default(),
        default_keymap: xkb_keymap,
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
        pending_container_render_data: Default::default(),
        pending_output_render_data: Default::default(),
        pending_float_layout: Default::default(),
        pending_float_titles: Default::default(),
        pending_input_popup_positioning: Default::default(),
        pending_toplevel_screencasts: Default::default(),
        pending_screencast_reallocs_or_reconfigures: Default::default(),
        dbus: Dbus::new(&engine, &ring, &run_toplevel),
        fdcloser: FdCloser::new(),
        logger: logger.clone(),
        connectors: Default::default(),
        outputs: Default::default(),
        drm_devs: Default::default(),
        status: Default::default(),
        idle: IdleState {
            input: Default::default(),
            change: Default::default(),
            timeout: Cell::new(Duration::from_secs(10 * 60)),
            timeout_changed: Default::default(),
            inhibitors: Default::default(),
            inhibitors_changed: Default::default(),
            backend_idle: Cell::new(true),
        },
        run_args,
        xwayland: XWaylandState {
            enabled: Cell::new(true),
            handler: Default::default(),
            queue: Default::default(),
            ipc_device_ids: Default::default(),
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
        default_gfx_api: Cell::new(GfxApi::OpenGl),
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
        security_context_acceptors: Default::default(),
        cursor_user_group_ids: Default::default(),
        cursor_user_ids: Default::default(),
        cursor_user_groups: Default::default(),
        cursor_user_group_hardware_cursor: Default::default(),
        input_device_group_ids: Default::default(),
        tablet_ids: Default::default(),
        tablet_tool_ids: Default::default(),
        tablet_pad_ids: Default::default(),
        damage_visualizer: DamageVisualizer::new(&engine),
        default_vrr_mode: Cell::new(VrrMode::NEVER),
        default_vrr_cursor_hz: Cell::new(None),
        default_tearing_mode: Cell::new(TearingMode::VARIANT_3),
        ei_acceptor: Default::default(),
        ei_acceptor_future: Default::default(),
        enable_ei_acceptor: Default::default(),
        ei_clients: EiClients::new(),
        slow_ei_clients: Default::default(),
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
        _portal = Some(engine.spawn(portal.spawn(engine.clone(), ring.clone(), logger.clone())));
    }
    let _compositor = engine.spawn(start_compositor3(state.clone(), test_future));
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

    let _geh = start_global_event_handlers(&state, &backend);
    state.start_xwayland();

    match backend.run().await {
        Err(e) => log::error!("Backend failed: {}", ErrorFmt(e.deref())),
        _ => log::error!("Backend stopped without an error"),
    }
    state.ring.stop();
}

fn load_config(state: &Rc<State>, #[allow(unused_variables)] for_test: bool) -> ConfigProxy {
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

fn start_global_event_handlers(
    state: &Rc<State>,
    backend: &Rc<dyn Backend>,
) -> Vec<SpawnedFuture<()>> {
    let eng = &state.eng;

    vec![
        eng.spawn(tasks::handle_backend_events(state.clone())),
        eng.spawn(tasks::handle_slow_clients(state.clone())),
        eng.spawn(tasks::handle_hardware_cursor_tick(state.clone())),
        eng.spawn2(Phase::Layout, container_layout(state.clone())),
        eng.spawn2(Phase::PostLayout, container_render_data(state.clone())),
        eng.spawn2(Phase::PostLayout, output_render_data(state.clone())),
        eng.spawn2(Phase::Layout, float_layout(state.clone())),
        eng.spawn2(Phase::PostLayout, float_titles(state.clone())),
        eng.spawn2(Phase::PostLayout, idle(state.clone(), backend.clone())),
        eng.spawn2(Phase::PostLayout, input_popup_positioning(state.clone())),
        eng.spawn2(Phase::Present, perform_toplevel_screencasts(state.clone())),
        eng.spawn2(Phase::PostLayout, perform_screencast_realloc(state.clone())),
        eng.spawn2(Phase::PostLayout, visualize_damage(state.clone())),
        eng.spawn(tasks::handle_slow_ei_clients(state.clone())),
    ]
}

async fn create_backend(
    state: &Rc<State>,
    #[allow(unused_variables)] test_future: Option<TestFuture>,
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
    });
    let connector = Rc::new(DummyOutput {
        id: state.connector_ids.next(),
    }) as Rc<dyn Connector>;
    let schedule = Rc::new(OutputSchedule::new(
        &state.ring,
        &state.eng,
        &connector,
        &persistent_state,
    ));
    let dummy_output = Rc::new(OutputNode {
        id: state.node_ids.next(),
        global: Rc::new(WlOutputGlobal::new(
            state.globals.name(),
            state,
            &Rc::new(ConnectorData {
                connector,
                handler: Cell::new(None),
                connected: Cell::new(true),
                name: "Dummy".to_string(),
                drm_dev: None,
                async_event: Default::default(),
            }),
            Vec::new(),
            &backend::Mode {
                width: 0,
                height: 0,
                refresh_rate_millihz: 0,
            },
            0,
            0,
            &output_id,
            &persistent_state,
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
        lock_surface: Default::default(),
        hardware_cursor: Default::default(),
        update_render_data_scheduled: Cell::new(false),
        screencasts: Default::default(),
        hardware_cursor_needs_render: Cell::new(false),
        screencopies: Default::default(),
        title_visible: Cell::new(false),
        schedule,
        latch_event: Default::default(),
    });
    let dummy_workspace = Rc::new(WorkspaceNode {
        id: state.node_ids.next(),
        state: state.clone(),
        is_dummy: true,
        output: CloneCell::new(dummy_output.clone()),
        position: Default::default(),
        container: Default::default(),
        stacked: Default::default(),
        seat_state: Default::default(),
        name: "dummy".to_string(),
        output_link: Default::default(),
        visible: Default::default(),
        fullscreen: Default::default(),
        visible_on_desired_output: Default::default(),
        desired_output: CloneCell::new(dummy_output.global.output_id.clone()),
        jay_workspaces: Default::default(),
        may_capture: Cell::new(false),
        has_capture: Cell::new(false),
        title_texture: Cell::new(None),
        attention_requests: Default::default(),
        render_highlight: Default::default(),
    });
    *dummy_workspace.output_link.borrow_mut() =
        Some(dummy_output.workspaces.add_last(dummy_workspace.clone()));
    dummy_output.show_workspace(&dummy_workspace);
    state.dummy_output.set(Some(dummy_output));
}

fn config_dir() -> Option<String> {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        Some(format!("{}/jay", xdg))
    } else if let Ok(home) = env::var("HOME") {
        Some(format!("{}/.config/jay", home))
    } else {
        log::warn!("Neither XDG_CONFIG_HOME nor HOME are set. Using default config.");
        None
    }
}

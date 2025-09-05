use {
    crate::{
        backend::{
            BackendConnectorState, BackendEvent, ConnectorEvent, ConnectorKernelId, MonitorInfo,
        },
        cmm::cmm_primaries::Primaries,
        format::XRGB8888,
        ifs::wl_output::OutputId,
        it::{test_backend::TestConnector, test_error::TestResult, testrun::TestRun},
        utils::numcell::NumCell,
        video::drm::ConnectorType,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client1 = run.create_client().await?;
    let win1 = client1.create_window().await?;
    win1.map2().await?;
    let surface = &win1.surface.server;

    let Some(dummy_output) = run.state.dummy_output.get() else {
        bail!("no dummy output");
    };

    let new_connector = Rc::new(TestConnector {
        id: run.state.connector_ids.next(),
        kernel_id: ConnectorKernelId {
            ty: ConnectorType::VGA,
            idx: 2,
        },
        events: Default::default(),
        feedback: Default::default(),
        idle: Default::default(),
        damage_calls: NumCell::new(0),
    });
    let new_monitor_info = MonitorInfo {
        modes: vec![],
        output_id: Rc::new(OutputId {
            connector: None,
            manufacturer: "jay".to_string(),
            model: "jay second connector".to_string(),
            serial_number: "".to_string(),
        }),
        width_mm: 0,
        height_mm: 0,
        non_desktop: false,
        non_desktop_effective: false,
        vrr_capable: false,
        eotfs: vec![],
        color_spaces: vec![],
        primaries: Primaries::SRGB,
        luminance: None,
        state: BackendConnectorState {
            serial: run.state.backend_connector_state_serials.next(),
            enabled: true,
            active: true,
            mode: Default::default(),
            non_desktop_override: None,
            vrr: false,
            tearing: false,
            format: XRGB8888,
            color_space: Default::default(),
            eotf: Default::default(),
        },
    };
    run.backend
        .state
        .backend_events
        .push(BackendEvent::NewConnector(new_connector.clone()));

    new_connector
        .events
        .send_event(ConnectorEvent::Connected(new_monitor_info.clone()));
    run.state.eng.yield_now().await;
    tassert_eq!(
        surface.get_output().global.connector.connector.id(),
        ds.connector.id
    );

    ds.connector.events.send_event(ConnectorEvent::Disconnected);
    run.state.eng.yield_now().await;
    tassert_eq!(
        surface.get_output().global.connector.connector.id(),
        new_connector.id
    );

    new_connector
        .events
        .send_event(ConnectorEvent::Disconnected);
    run.state.eng.yield_now().await;
    tassert_eq!(
        surface.get_output().global.connector.connector.id(),
        dummy_output.global.connector.connector.id()
    );

    new_connector
        .events
        .send_event(ConnectorEvent::Connected(new_monitor_info.clone()));
    run.state.eng.yield_now().await;
    tassert_eq!(
        surface.get_output().global.connector.connector.id(),
        new_connector.id
    );

    ds.connector.events.send_event(ConnectorEvent::Connected(
        run.backend.default_monitor_info.clone(),
    ));
    run.state.eng.yield_now().await;
    tassert_eq!(
        surface.get_output().global.connector.connector.id(),
        ds.connector.id
    );

    Ok(())
}

use {
    crate::{
        backend::{BackendEvent, ConnectorEvent, ConnectorKernelId, Mode, MonitorInfo},
        it::{test_backend::TestConnector, test_error::TestResult, testrun::TestRun},
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
    });
    let new_monitor_info = MonitorInfo {
        modes: vec![],
        manufacturer: "jay".to_string(),
        product: "jay second connector".to_string(),
        serial_number: "".to_string(),
        initial_mode: Mode {
            width: 400,
            height: 400,
            refresh_rate_millihz: 60000,
        },
        width_mm: 0,
        height_mm: 0,
        non_desktop: false,
        vrr_capable: false,
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

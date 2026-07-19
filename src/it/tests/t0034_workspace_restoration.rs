use crate::backend::BackendConnectorState;
use crate::backend::BackendDrmDevice;
use crate::backend::BackendEvent;
use crate::backend::ConnectorEvent;
use crate::backend::ConnectorKernelId;
use crate::backend::MonitorInfo;
use crate::cmm::cmm_primaries::Primaries;
use crate::format::XRGB8888;
use crate::ifs::wl_output::OutputId;
use crate::it::test_backend::TestConnector;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::utils::numcell::NumCell;
use crate::video::drm::ConnectorType;
use std::cell::RefCell;
use std::rc::Rc;

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

    let bcs = BackendConnectorState {
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
        gamma_lut: Default::default(),
    };
    let new_connector = Rc::new(TestConnector {
        id: run.state.connector_ids.next(),
        drm_dev_id: run.backend.default_drm_dev.id(),
        kernel_id: ConnectorKernelId {
            ty: ConnectorType::VGA,
            idx: 2,
        },
        events: Default::default(),
        idle: Default::default(),
        damage_calls: NumCell::new(0),
        state: RefCell::new(bcs.clone()),
        scanout_formats: Default::default(),
    });
    let new_monitor_info = MonitorInfo {
        modes: Some(vec![]),
        output_id: OutputId::new("", "jay", "jay second connector", ""),
        width_mm: 0,
        height_mm: 0,
        non_desktop: false,
        non_desktop_effective: false,
        vrr_capable: false,
        eotfs: vec![],
        color_spaces: vec![],
        primaries: Primaries::SRGB,
        luminance: None,
        state: bcs,
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

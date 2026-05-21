use {
    crate::{
        backend::{BackendConnectorState, BackendEvent, ConnectorEvent, MonitorInfo},
        cmm::cmm_primaries::Primaries,
        format::XRGB8888,
        ifs::wl_output::OutputId,
        it::{test_backend::TestConnector, test_error::TestResult, testrun::TestRun},
        tree::OutputNode,
    },
    std::rc::Rc,
};

pub async fn create_output(
    run: &Rc<TestRun>,
    connector_name: &str,
    connector_idx: u32,
) -> (Rc<TestConnector>, MonitorInfo) {
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
    let connector = Rc::new(TestConnector::new(
        run.state.connector_ids.next(),
        connector_idx,
        bcs.clone(),
    ));
    let output_name = format!("jay {connector_name} {}", connector.id.raw());
    let output_serial = connector.id.raw().to_string();
    let monitor_info = MonitorInfo {
        modes: Some(vec![]),
        output_id: OutputId::new("", "jay", output_name, output_serial),
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
        .push(BackendEvent::NewConnector(connector.clone()));
    connector
        .events
        .send_event(ConnectorEvent::Connected(monitor_info.clone()));
    run.sync().await;
    (connector, monitor_info)
}

pub fn get_output(run: &Rc<TestRun>, connector: &TestConnector) -> TestResult<Rc<OutputNode>> {
    match find_output(run, connector) {
        Some(output) => Ok(output),
        None => bail!("Output for connector {} not found", connector.id.raw()),
    }
}

fn find_output(run: &Rc<TestRun>, connector: &TestConnector) -> Option<Rc<OutputNode>> {
    run.state
        .root
        .outputs
        .lock()
        .values()
        .find(|output| output.global.connector.connector.id() == connector.id)
        .cloned()
}

pub async fn wait_for_output_removal(run: &Rc<TestRun>, connector: &TestConnector) -> TestResult {
    for _ in 0..10 {
        if run.state.root.outputs.not_contains(&connector.id) {
            return Ok(());
        }
        run.state.eng.yield_now().await;
    }
    bail!("Output {} was not removed", connector.id.raw())
}

pub async fn wait_for_output_addition(run: &Rc<TestRun>, connector: &TestConnector) -> TestResult {
    for _ in 0..10 {
        if find_output(run, connector).is_some() {
            return Ok(());
        }
        run.state.eng.yield_now().await;
    }
    bail!("Output {} was not added", connector.id.raw())
}

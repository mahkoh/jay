use {
    crate::{
        backend::{
            BackendConnectorState, BackendEvent, ConnectorEvent, ConnectorKernelId, MonitorInfo,
        },
        cmm::cmm_primaries::Primaries,
        format::XRGB8888,
        ifs::wl_output::OutputId,
        it::{
            test_backend::TestConnector,
            test_error::TestResult,
            testrun::{DefaultSetup, TestRun},
        },
        utils::numcell::NumCell,
        video::drm::ConnectorType,
    },
    jay_config::workspace::WorkspaceEmptyBehavior,
    std::{cell::RefCell, rc::Rc},
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    test_preserve(&run, &ds).await?;
    test_destroy_on_leave_timing(&run, &ds).await?;
    test_hide_on_leave_timing(&run, &ds).await?;
    test_hide_on_leave(&run, &ds).await?;
    test_destroy(&run, &ds).await?;
    test_hide(&run, &ds).await?;
    test_restore_output_preference(&run, &ds).await?;
    Ok(())
}

// preserve: switching away from an empty workspace keeps it listed and alive.
async fn test_preserve(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "p1")?;
    let after_p1 = run.state.workspaces.len();
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Preserve)?;
    run.cfg.show_workspace(ds.seat.id(), "p2")?;
    tassert_eq!(run.state.workspaces.len(), after_p1 + 1);
    tassert!(run.state.workspaces.contains("p1"));
    tassert!(run.state.workspaces.contains("p2"));
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "p1"));
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "p2"));
    Ok(())
}

// destroy-on-leave timing: leaving a non-empty workspace must not destroy it later if it becomes
// empty while inactive. It is destroyed only if it is empty at the moment you switch away.
async fn test_destroy_on_leave_timing(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "dol1")?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::DestroyOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "dol2")?;
    tassert!(run.state.workspaces.contains("dol1"));
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(run.state.workspaces.contains("dol1"));
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "dol1"));
    run.cfg.show_workspace(ds.seat.id(), "dol1")?;
    tassert!(run.state.workspaces.contains("dol1"));
    run.cfg.show_workspace(ds.seat.id(), "dol2")?;
    tassert!(run.state.workspaces.not_contains("dol1"));
    tassert!(!ds.output.workspaces.iter().any(|ws| ws.name == "dol1"));
    Ok(())
}

// hide-on-leave timing: leaving a non-empty workspace must not hide it later if it becomes empty
// while inactive. It is hidden only if it is empty at the moment you switch away.
async fn test_hide_on_leave_timing(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "hol1")?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "hol2")?;
    let hol1 = run.state.workspaces.get("hol1").unwrap();
    tassert!(!hol1.hidden.get());
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "hol1"));
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    let hol1 = run.state.workspaces.get("hol1").unwrap();
    tassert!(!hol1.hidden.get());
    tassert!(run.state.workspaces.contains("hol1"));
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "hol1"));
    run.cfg.show_workspace(ds.seat.id(), "hol1")?;
    run.cfg.show_workspace(ds.seat.id(), "hol2")?;
    let hol1 = run.state.workspaces.get("hol1").unwrap();
    tassert!(hol1.hidden.get());
    tassert!(run.state.workspaces.contains("hol1"));
    tassert!(!ds.output.workspaces.iter().any(|ws| ws.name == "hol1"));
    run.cfg.show_workspace(ds.seat.id(), "hol1")?;
    let hol1 = run.state.workspaces.get("hol1").unwrap();
    tassert!(!hol1.hidden.get());
    tassert!(!hol1.output.get().is_dummy);
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "hol1"));
    Ok(())
}

// hide-on-leave: switching away hides an empty workspace and showing it by name restores it.
async fn test_hide_on_leave(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "h1")?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "h2")?;
    let h1 = run.state.workspaces.get("h1").unwrap();
    tassert!(h1.hidden.get());
    tassert!(run.state.workspaces.contains("h1"));
    tassert!(!ds.output.workspaces.iter().any(|ws| ws.name == "h1"));
    run.cfg.show_workspace(ds.seat.id(), "h1")?;
    let h1 = run.state.workspaces.get("h1").unwrap();
    tassert!(!h1.hidden.get());
    tassert!(!h1.output.get().is_dummy);
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "h1"));
    Ok(())
}

// destroy: when an inactive workspace becomes empty, it is destroyed immediately.
async fn test_destroy(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "d1")?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Destroy)?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg.show_workspace(ds.seat.id(), "d2")?;
    tassert!(run.state.workspaces.contains("d1"));
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(run.state.workspaces.not_contains("d1"));
    Ok(())
}

// hide: when an inactive workspace becomes empty, it becomes hidden and can be restored by name.
async fn test_hide(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "hi1")?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg.show_workspace(ds.seat.id(), "hi2")?;
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    let hi1 = run.state.workspaces.get("hi1").unwrap();
    tassert!(hi1.hidden.get());
    tassert!(run.state.workspaces.contains("hi1"));
    tassert!(!ds.output.workspaces.iter().any(|ws| ws.name == "hi1"));
    run.cfg.show_workspace(ds.seat.id(), "hi1")?;
    let hi1 = run.state.workspaces.get("hi1").unwrap();
    tassert!(!hi1.hidden.get());
    tassert!(!hi1.output.get().is_dummy);
    tassert!(ds.output.workspaces.iter().any(|ws| ws.name == "hi1"));
    Ok(())
}

// restore output preference: hidden workspaces reopen on the connected output matching desired_output.
async fn test_restore_output_preference(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    // Create a second output so we can set desired_output to a non-default connected output.
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
        kernel_id: ConnectorKernelId {
            ty: ConnectorType::VGA,
            idx: 2,
        },
        events: Default::default(),
        feedback: Default::default(),
        idle: Default::default(),
        damage_calls: NumCell::new(0),
        state: RefCell::new(bcs.clone()),
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

    // Hotplug the connector so the compositor creates an OutputNode for it.
    run.backend
        .state
        .backend_events
        .push(BackendEvent::NewConnector(new_connector.clone()));
    new_connector
        .events
        .send_event(ConnectorEvent::Connected(new_monitor_info));
    run.sync().await;

    // Find the new OutputNode by connector id.
    let output2 = run
        .state
        .root
        .outputs
        .lock()
        .values()
        .find(|o| o.global.connector.connector.id() == new_connector.id)
        .unwrap()
        .clone();
    run.cfg.show_workspace(ds.seat.id(), "r1")?;
    let r1 = run.state.workspaces.get("r1").unwrap();

    // Move the workspace to output2, updating desired_output, and ensure it is attached there.
    run.state.move_ws_to_output(&r1, &output2);
    run.state.show_workspace2(None, &output2, &r1);
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;

    // Switching away from an empty r1 should hide it and keep desired_output pointing at output2.
    let other = output2.create_workspace("other");
    run.state.show_workspace2(None, &output2, &other);
    tassert!(r1.hidden.get());

    // Restore by name via a different output argument; restoration must still prefer desired_output.
    run.state.show_workspace2(None, &ds.output, &r1);
    tassert!(!r1.hidden.get());
    tassert_eq!(r1.output.get().id, output2.id);
    tassert!(output2.workspaces.iter().any(|ws| ws.name == "r1"));
    Ok(())
}

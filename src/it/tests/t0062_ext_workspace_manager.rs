use {
    crate::{
        backend::ConnectorEvent,
        it::{
            test_client::TestClient,
            test_error::TestResult,
            test_ifs::{
                test_ext_workspace_group_handle::TestExtWorkspaceGroupHandle,
                test_ext_workspace_handle::TestExtWorkspaceHandle,
                test_ext_workspace_manager::TestExtWorkspaceManager,
            },
            test_utils::test_output_setup::{
                create_output, get_output, wait_for_output_addition, wait_for_output_removal,
            },
            testrun::{DefaultSetup, TestRun},
        },
        object::ObjectId,
    },
    jay_config::workspace::WorkspaceEmptyBehavior,
    std::rc::Rc,
};

const STATE_ACTIVE: u32 = 1;
const STATE_HIDDEN: u32 = 4;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    test_visible_workspace_enumeration(&run, &ds).await?;
    test_hide_and_activate_protocol(&run, &ds).await?;
    test_create_workspace_respects_empty_behavior(&run, &ds).await?;
    test_assign_hidden_workspace_protocol(&run, &ds).await?;
    test_move_hidden_workspace_protocol(&run, &ds).await?;
    test_hidden_workspace_reconnect_protocol(&run, &ds).await?;
    test_destroy_protocol(&run, &ds).await?;
    Ok(())
}

async fn bind_workspace_manager(
    client: &Rc<TestClient>,
) -> TestResult<Rc<TestExtWorkspaceManager>> {
    let manager = client.registry.get_workspace_manager().await?;
    client.sync().await;
    Ok(manager)
}

fn workspace(manager: &TestExtWorkspaceManager, name: &str) -> Rc<TestExtWorkspaceHandle> {
    manager.workspace_by_name(name).unwrap()
}

fn group_with_workspace(
    manager: &TestExtWorkspaceManager,
    workspace: &TestExtWorkspaceHandle,
) -> Rc<TestExtWorkspaceGroupHandle> {
    if let Some(group_id) = workspace.current_group.get() {
        return manager
            .groups
            .borrow()
            .iter()
            .find(|group| {
                let id: ObjectId = group.id.into();
                id == group_id
            })
            .cloned()
            .unwrap();
    }
    let workspace_id: ObjectId = workspace.id.into();
    manager
        .groups
        .borrow()
        .iter()
        .find(|group| group.workspaces.borrow().contains(&workspace_id))
        .cloned()
        .unwrap()
}

async fn test_visible_workspace_enumeration(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "ext-visible")?;
    let client = run.create_client().await?;
    let manager = bind_workspace_manager(&client).await?;
    let ws = workspace(&manager, "ext-visible");
    tassert!(!ws.removed.get());
    tassert_eq!(ws.state.get() & STATE_ACTIVE, STATE_ACTIVE);
    tassert_eq!(ws.state.get() & STATE_HIDDEN, 0);
    tassert!(ws.current_group.get().is_some());
    Ok(())
}

async fn test_hide_and_activate_protocol(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "ext-hide-a")?;
    let client = run.create_client().await?;
    let manager = bind_workspace_manager(&client).await?;
    let group = group_with_workspace(&manager, &workspace(&manager, "ext-hide-a"));
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "ext-hide-b")?;
    client.sync().await;
    let hidden = workspace(&manager, "ext-hide-a");
    tassert_eq!(hidden.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert_eq!(hidden.current_group.get(), Some(group.id.into()));
    tassert!(group.workspaces.borrow().contains(&hidden.id.into()));
    hidden.activate()?;
    manager.commit()?;
    client.sync().await;
    let restored = workspace(&manager, "ext-hide-a");
    tassert_eq!(restored.state.get() & STATE_HIDDEN, 0);
    tassert_eq!(restored.current_group.get(), Some(group.id.into()));
    Ok(())
}

async fn test_create_workspace_respects_empty_behavior(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg.show_workspace(ds.seat.id(), "ext-create-active")?;
    let client = run.create_client().await?;
    let manager = bind_workspace_manager(&client).await?;
    let group = group_with_workspace(&manager, &workspace(&manager, "ext-create-active"));
    group.create_workspace("ext-create-hidden")?;
    manager.commit()?;
    client.sync().await;
    let created = workspace(&manager, "ext-create-hidden");
    tassert_eq!(created.state.get() & STATE_ACTIVE, 0);
    tassert_eq!(created.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Destroy)?;
    group.create_workspace("ext-create-destroyed")?;
    manager.commit()?;
    client.sync().await;
    let created = workspace(&manager, "ext-create-destroyed");
    tassert!(created.removed.get());
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Preserve)?;
    Ok(())
}

async fn test_assign_hidden_workspace_protocol(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    let (connector2, _) = create_output(run, "ext assign", 2).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg.show_workspace(ds.seat.id(), "ext-assign-target")?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "ext-assign-hidden", &output2)?;
    let client = run.create_client().await?;
    let manager = bind_workspace_manager(&client).await?;
    let target_group = group_with_workspace(&manager, &workspace(&manager, "ext-assign-target"));
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "ext-assign-other", &output2)?;
    client.sync().await;
    let hidden = workspace(&manager, "ext-assign-hidden");
    tassert_eq!(hidden.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert!(hidden.current_group.get().is_some());
    hidden.assign(&target_group)?;
    manager.commit()?;
    client.sync().await;
    let restored = workspace(&manager, "ext-assign-hidden");
    tassert_eq!(restored.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert_eq!(restored.current_group.get(), Some(target_group.id.into()));
    Ok(())
}

async fn test_move_hidden_workspace_protocol(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    let (connector2, _) = create_output(run, "ext move", 4).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "ext-move-target")?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "ext-move-hidden", &output2)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "ext-move-other", &output2)?;
    run.cfg
        .move_workspace_to_output("ext-move-hidden", &ds.output)?;
    let late_client = run.create_client().await?;
    let late_manager = bind_workspace_manager(&late_client).await?;
    let target_group =
        group_with_workspace(&late_manager, &workspace(&late_manager, "ext-move-target"));
    let hidden = workspace(&late_manager, "ext-move-hidden");
    tassert_eq!(hidden.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert_eq!(hidden.current_group.get(), Some(target_group.id.into()));
    tassert!(target_group.workspaces.borrow().contains(&hidden.id.into()));
    Ok(())
}

async fn test_hidden_workspace_reconnect_protocol(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    let client = run.create_client().await?;
    let manager = bind_workspace_manager(&client).await?;
    let (connector2, monitor_info) = create_output(run, "ext reconnect", 3).await;
    let output2 = get_output(run, &connector2)?;
    client.sync().await;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "ext-reconnect-a", &output2)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "ext-reconnect-b", &output2)?;
    client.sync().await;
    let hidden = workspace(&manager, "ext-reconnect-a");
    tassert_eq!(hidden.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert!(hidden.current_group.get().is_some());
    connector2.events.send_event(ConnectorEvent::Disconnected);
    wait_for_output_removal(run, &connector2).await?;
    connector2
        .events
        .send_event(ConnectorEvent::Connected(monitor_info));
    wait_for_output_addition(run, &connector2).await?;
    client.sync().await;
    let hidden = workspace(&manager, "ext-reconnect-a");
    tassert_eq!(hidden.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert!(hidden.current_group.get().is_some());
    let late_client = run.create_client().await?;
    let late_manager = bind_workspace_manager(&late_client).await?;
    let late_hidden = workspace(&late_manager, "ext-reconnect-a");
    tassert_eq!(late_hidden.state.get() & STATE_HIDDEN, STATE_HIDDEN);
    tassert!(late_hidden.current_group.get().is_some());
    Ok(())
}

async fn test_destroy_protocol(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg.show_workspace(ds.seat.id(), "ext-destroy-a")?;
    let client = run.create_client().await?;
    let manager = bind_workspace_manager(&client).await?;
    let win = client.create_window().await?;
    win.map().await?;
    let doomed = workspace(&manager, "ext-destroy-a");
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Destroy)?;
    run.cfg.show_workspace(ds.seat.id(), "ext-destroy-b")?;
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(doomed.removed.get());
    Ok(())
}

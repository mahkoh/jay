use {
    crate::{
        backend::ConnectorEvent,
        it::{
            test_error::TestResult,
            test_utils::test_output_setup::{
                create_output, get_output, wait_for_output_addition, wait_for_output_removal,
            },
            testrun::{DefaultSetup, TestRun},
        },
    },
    jay_config::{video::Connector, workspace::WorkspaceEmptyBehavior},
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    test_empty_workspace_modes(&run, &ds).await?;
    test_per_workspace_empty_behavior(&run, &ds).await?;
    test_leave_nonempty_then_empty_keeps_workspace(&run, &ds).await?;
    test_move_empty_workspace_to_occupied_output(&run, &ds).await?;
    test_hidden_workspace_listing_and_watchers(&run, &ds).await?;
    test_move_hidden_workspace_remembers_output(&run, &ds).await?;
    test_hidden_workspace_restores_on_requested_output(&run, &ds).await?;
    test_hidden_workspace_reconnect_behavior(&run, &ds).await?;
    test_hidden_workspace_unplug_restore_behavior(&run, &ds).await?;
    test_float_workspace_move_enforces_empty_behavior(&run, &ds).await?;
    test_seat_assignment_without_focused_window_keeps_hidden_workspace(&run, &ds).await?;
    test_workspace_assignment_restores_hidden_workspace(&run, &ds).await?;
    Ok(())
}

fn connector(output: &crate::tree::OutputNode) -> Connector {
    Connector(output.global.connector.connector.id().raw() as _)
}

async fn test_leave_nonempty_then_empty_keeps_workspace(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::DestroyOnLeave)?;
    run.cfg
        .show_workspace(ds.seat.id(), "leave-nonempty-dol-a")?;
    let destroy_on_leave_workspace = run.cfg.get_workspace("leave-nonempty-dol-a")?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .show_workspace(ds.seat.id(), "leave-nonempty-dol-b")?;
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(
        run.cfg
            .get_workspaces()?
            .contains(&destroy_on_leave_workspace)
    );
    tassert_eq!(
        run.cfg.get_workspace_connector("leave-nonempty-dol-a")?,
        connector(&ds.output)
    );

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg
        .show_workspace(ds.seat.id(), "leave-nonempty-hol-a")?;
    let hide_on_leave_workspace = run.cfg.get_workspace("leave-nonempty-hol-a")?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .show_workspace(ds.seat.id(), "leave-nonempty-hol-b")?;
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(run.cfg.get_workspaces()?.contains(&hide_on_leave_workspace));
    tassert_eq!(
        run.cfg.get_workspace_connector("leave-nonempty-hol-a")?,
        connector(&ds.output)
    );
    Ok(())
}

async fn test_empty_workspace_modes(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Preserve)?;
    run.cfg.show_workspace(ds.seat.id(), "preserve-a")?;
    let preserve = run.cfg.get_workspace("preserve-a")?;
    run.cfg.show_workspace(ds.seat.id(), "preserve-b")?;
    let preserve_b = run.cfg.get_workspace("preserve-b")?;
    tassert!(run.cfg.get_workspaces()?.contains(&preserve));
    tassert!(run.cfg.get_workspaces()?.contains(&preserve_b));
    tassert_eq!(
        run.cfg.get_workspace_connector("preserve-b")?,
        connector(&ds.output)
    );

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::DestroyOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "destroy-on-leave-a")?;
    let destroy_on_leave = run.cfg.get_workspace("destroy-on-leave-a")?;
    run.cfg.show_workspace(ds.seat.id(), "destroy-on-leave-b")?;
    let destroy_on_leave_b = run.cfg.get_workspace("destroy-on-leave-b")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&destroy_on_leave));
    tassert!(run.cfg.get_workspaces()?.contains(&destroy_on_leave_b));
    tassert_eq!(
        run.cfg.get_workspace_connector("destroy-on-leave-b")?,
        connector(&ds.output)
    );

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Destroy)?;
    run.cfg.show_workspace(ds.seat.id(), "destroy-leave-a")?;
    let destroy_leave = run.cfg.get_workspace("destroy-leave-a")?;
    run.cfg.show_workspace(ds.seat.id(), "destroy-leave-b")?;
    let destroy_leave_b = run.cfg.get_workspace("destroy-leave-b")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&destroy_leave));
    tassert!(run.cfg.get_workspaces()?.contains(&destroy_leave_b));
    tassert_eq!(
        run.cfg.get_workspace_connector("destroy-leave-b")?,
        connector(&ds.output)
    );

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg.show_workspace(ds.seat.id(), "hide-leave-a")?;
    let hide_leave = run.cfg.get_workspace("hide-leave-a")?;
    run.cfg.show_workspace(ds.seat.id(), "hide-leave-b")?;
    let hide_leave_b = run.cfg.get_workspace("hide-leave-b")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&hide_leave));
    tassert!(run.cfg.get_workspaces()?.contains(&hide_leave_b));
    tassert_eq!(
        run.cfg.get_workspace_connector("hide-leave-b")?,
        connector(&ds.output)
    );
    run.cfg.show_workspace(ds.seat.id(), "hide-leave-a")?;
    tassert!(run.cfg.get_workspaces()?.contains(&hide_leave));

    let client = run.create_client().await?;
    run.cfg.show_workspace(ds.seat.id(), "destroy-a")?;
    let destroy = run.cfg.get_workspace("destroy-a")?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Destroy)?;
    run.cfg.show_workspace(ds.seat.id(), "destroy-b")?;
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(!run.cfg.get_workspaces()?.contains(&destroy));

    run.cfg.show_workspace(ds.seat.id(), "hide-a")?;
    let hide = run.cfg.get_workspace("hide-a")?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg.show_workspace(ds.seat.id(), "hide-b")?;
    win.tl.core.destroy()?;
    win.xdg.destroy()?;
    win.surface.viewport.destroy()?;
    win.surface.surface.destroy()?;
    client.sync().await;
    tassert!(!run.cfg.get_workspaces()?.contains(&hide));
    run.cfg.show_workspace(ds.seat.id(), "hide-a")?;
    tassert!(run.cfg.get_workspaces()?.contains(&hide));
    Ok(())
}

async fn test_per_workspace_empty_behavior(run: &Rc<TestRun>, ds: &DefaultSetup) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Preserve)?;
    let destroy = run.cfg.get_workspace("per-workspace-destroy")?;
    run.cfg
        .set_workspace_empty_behavior_override(destroy, WorkspaceEmptyBehavior::DestroyOnLeave)?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-destroy")?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-destroy-b")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&destroy));
    run.cfg.clear_workspace_empty_behavior_override(destroy)?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-destroy")?;
    let destroy = run.cfg.get_workspace("per-workspace-destroy")?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-destroy-b")?;
    tassert!(run.cfg.get_workspaces()?.contains(&destroy));

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-preserve")?;
    let preserve = run.cfg.get_workspace("per-workspace-preserve")?;
    run.cfg
        .set_workspace_empty_behavior_override(preserve, WorkspaceEmptyBehavior::Preserve)?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-preserve-b")?;
    tassert!(run.cfg.get_workspaces()?.contains(&preserve));
    run.cfg.clear_workspace_empty_behavior_override(preserve)?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-preserve")?;
    let preserve = run.cfg.get_workspace("per-workspace-preserve")?;
    run.cfg
        .show_workspace(ds.seat.id(), "per-workspace-preserve-b")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&preserve));
    Ok(())
}

async fn test_move_empty_workspace_to_occupied_output(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    let (connector2, _) = create_output(run, "occupied output", 5).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "occupied", &output2)?;

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg.show_workspace(ds.seat.id(), "move-empty-hide")?;
    let hide = run.cfg.get_workspace("move-empty-hide")?;
    run.cfg
        .move_workspace_to_output("move-empty-hide", &output2)?;
    tassert!(!run.cfg.get_workspaces()?.contains(&hide));
    run.cfg.show_workspace(ds.seat.id(), "move-empty-hide")?;
    tassert!(run.cfg.get_workspaces()?.contains(&hide));
    tassert_eq!(
        run.cfg.get_workspace_connector("move-empty-hide")?,
        connector(&output2)
    );

    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Destroy)?;
    run.cfg.show_workspace(ds.seat.id(), "move-empty-destroy")?;
    let destroy = run.cfg.get_workspace("move-empty-destroy")?;
    run.cfg
        .move_workspace_to_output("move-empty-destroy", &output2)?;
    tassert!(!run.cfg.get_workspaces()?.contains(&destroy));
    Ok(())
}

async fn test_hidden_workspace_listing_and_watchers(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "watch-a")?;
    let watch = run.cfg.get_workspace("watch-a")?;
    let client = run.create_client().await?;
    let watcher = client.jc.watch_workspaces()?;
    client.sync().await;
    let watched = watcher.live_workspace_by_name("watch-a").unwrap();
    run.cfg.show_workspace(ds.seat.id(), "watch-b")?;
    client.sync().await;
    tassert!(!run.cfg.get_workspaces()?.contains(&watch));
    tassert_eq!(run.cfg.get_workspace_connector("watch-a")?, Connector(0));
    tassert!(watched.destroyed.get());
    tassert!(watcher.live_workspace_by_name("watch-a").is_none());
    let late_client = run.create_client().await?;
    let late_watcher = late_client.jc.watch_workspaces()?;
    late_client.sync().await;
    tassert!(late_watcher.workspace_by_name("watch-a").is_none());
    run.cfg.show_workspace(ds.seat.id(), "watch-a")?;
    client.sync().await;
    late_client.sync().await;
    tassert!(run.cfg.get_workspaces()?.contains(&watch));
    tassert!(watcher.live_workspace_by_name("watch-a").is_some());
    tassert!(late_watcher.live_workspace_by_name("watch-a").is_some());
    Ok(())
}

async fn test_move_hidden_workspace_remembers_output(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    let (connector2, _) = create_output(run, "hidden output", 2).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "output-a", &output2)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "output-b", &output2)?;
    tassert_eq!(run.cfg.get_workspace_connector("output-a")?, Connector(0));
    run.cfg.move_workspace_to_output("output-a", &ds.output)?;
    tassert_eq!(run.cfg.get_workspace_connector("output-a")?, Connector(0));
    run.cfg.show_workspace(ds.seat.id(), "output-a")?;
    tassert_eq!(
        run.cfg.get_workspace_connector("output-a")?,
        connector(&ds.output)
    );
    Ok(())
}

async fn test_hidden_workspace_restores_on_requested_output(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    let (connector2, _) = create_output(run, "show move output", 8).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg.show_workspace(ds.seat.id(), "show-move-a")?;
    run.cfg.show_workspace(ds.seat.id(), "show-move-b")?;
    tassert_eq!(
        run.cfg.get_workspace_connector("show-move-a")?,
        Connector(0)
    );
    run.cfg
        .show_workspace_move_to_output(ds.seat.id(), "show-move-a", &output2)?;
    tassert_eq!(
        run.cfg.get_workspace_connector("show-move-a")?,
        connector(&output2)
    );
    Ok(())
}

async fn test_hidden_workspace_reconnect_behavior(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    let (connector2, monitor_info) = create_output(run, "reconnect output", 3).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "reconnect-a", &output2)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "reconnect-b", &output2)?;
    tassert_eq!(
        run.cfg.get_workspace_connector("reconnect-a")?,
        Connector(0)
    );
    connector2.events.send_event(ConnectorEvent::Disconnected);
    wait_for_output_removal(run, &connector2).await?;
    connector2
        .events
        .send_event(ConnectorEvent::Connected(monitor_info));
    wait_for_output_addition(run, &connector2).await?;
    run.cfg.show_workspace(ds.seat.id(), "reconnect-a")?;
    tassert_eq!(
        run.cfg.get_workspace_connector("reconnect-a")?,
        connector(&output2)
    );
    Ok(())
}

async fn test_hidden_workspace_unplug_restore_behavior(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    let (connector2, monitor_info) = create_output(run, "unplug output", 4).await;
    let output2 = get_output(run, &connector2)?;
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg
        .show_workspace_on(ds.seat.id(), "unplug-a", &output2)?;
    tassert_eq!(
        run.cfg.get_workspace_connector("unplug-a")?,
        connector(&output2)
    );
    connector2.events.send_event(ConnectorEvent::Disconnected);
    wait_for_output_removal(run, &connector2).await?;
    connector2
        .events
        .send_event(ConnectorEvent::Connected(monitor_info));
    wait_for_output_addition(run, &connector2).await?;
    run.cfg.show_workspace(ds.seat.id(), "unplug-a")?;
    tassert_eq!(
        run.cfg.get_workspace_connector("unplug-a")?,
        connector(&output2)
    );
    Ok(())
}

async fn test_float_workspace_move_enforces_empty_behavior(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::Hide)?;
    run.cfg.show_workspace(ds.seat.id(), "float-old")?;
    let old = run.cfg.get_workspace("float-old")?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    let window = run.cfg.get_workspace_window("float-old")?;
    run.cfg.set_window_floating(window, true)?;
    run.cfg.show_workspace(ds.seat.id(), "float-new")?;
    run.cfg.set_window_workspace(window, "float-new")?;
    client.sync().await;
    tassert!(!run.cfg.get_workspaces()?.contains(&old));
    Ok(())
}

async fn test_workspace_assignment_restores_hidden_workspace(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg
        .show_workspace(ds.seat.id(), "assign-window-hidden")?;
    run.cfg
        .show_workspace(ds.seat.id(), "assign-window-active")?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    let window = run.cfg.get_workspace_window("assign-window-active")?;
    run.cfg
        .set_window_workspace(window, "assign-window-hidden")?;
    tassert_eq!(
        run.cfg.get_workspace_connector("assign-window-hidden")?,
        connector(&ds.output)
    );

    run.cfg.show_workspace(ds.seat.id(), "assign-seat-hidden")?;
    run.cfg.show_workspace(ds.seat.id(), "assign-seat-active")?;
    let win = client.create_window().await?;
    win.map().await?;
    run.cfg
        .set_seat_workspace(ds.seat.id(), "assign-seat-hidden")?;
    tassert_eq!(
        run.cfg.get_workspace_connector("assign-seat-hidden")?,
        connector(&ds.output)
    );
    Ok(())
}

async fn test_seat_assignment_without_focused_window_keeps_hidden_workspace(
    run: &Rc<TestRun>,
    ds: &DefaultSetup,
) -> TestResult {
    run.cfg
        .set_workspace_empty_behavior(WorkspaceEmptyBehavior::HideOnLeave)?;
    run.cfg
        .show_workspace(ds.seat.id(), "assign-no-window-hidden")?;
    let hidden = run.cfg.get_workspace("assign-no-window-hidden")?;
    run.cfg
        .show_workspace(ds.seat.id(), "assign-no-window-active")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&hidden));
    run.cfg
        .set_seat_workspace(ds.seat.id(), "assign-no-window-hidden")?;
    tassert!(!run.cfg.get_workspaces()?.contains(&hidden));
    tassert_eq!(
        run.cfg.get_workspace_connector("assign-no-window-hidden")?,
        Connector(0)
    );
    Ok(())
}

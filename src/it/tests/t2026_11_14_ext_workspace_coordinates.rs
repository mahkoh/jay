use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_ext_workspace_manager::TestExtWorkspaceManager;
use crate::it::testrun::TestRun;
use crate::tree::OutputNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::WorkspaceDisplayOrder;
use crate::tree::WsMoveConfig;
use crate::tree::move_ws_to_output;
use std::rc::Rc;

testcase!();

fn output_order(output: &OutputNode) -> Vec<String> {
    output
        .workspaces
        .iter_valid(LiveTL)
        .map(|ws| ws.name.to_string())
        .collect()
}

fn announced_order(manager: &TestExtWorkspaceManager) -> Vec<String> {
    let mut workspaces: Vec<_> = manager
        .workspaces
        .borrow()
        .iter()
        .filter(|ws| !ws.removed.get())
        .map(|ws| (ws.coordinates.borrow().clone(), ws.name.borrow().clone()))
        .collect();
    workspaces.sort();
    workspaces.into_iter().map(|(_, name)| name).collect()
}

fn assert_coordinates_match(manager: &TestExtWorkspaceManager, output: &OutputNode) -> TestResult {
    let order = output_order(output);
    tassert_eq!(announced_order(manager), order.clone());
    for (idx, name) in order.iter().enumerate() {
        let Some(ws) = manager.workspace(name) else {
            bail!("Workspace {name} was not announced");
        };
        tassert_eq!(ws.coordinates.borrow().clone(), vec![idx as u32]);
    }
    Ok(())
}

/// Test that ext-workspace coordinates track the position of a workspace in its output.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    let client = run.create_client()?;
    let manager = client.new_workspace_manager();
    client.sync().await;
    client.sync().await;
    assert_coordinates_match(&manager, &ds.output)?;

    let c = ds.output.create_normal_workspace("c");
    let a = ds.output.create_normal_workspace("a");
    client.sync().await;
    client.sync().await;
    tassert_eq!(output_order(&ds.output).len(), 3);
    assert_coordinates_match(&manager, &ds.output)?;

    let first = ds.output.workspaces.iter_valid(LiveTL).next().unwrap();
    let config = WsMoveConfig {
        make_visible_always: false,
        make_visible_if_empty: false,
        source_is_destroyed: false,
        before: Some(first.item.clone()),
    };
    move_ws_to_output(&a, &ds.output, config);
    client.sync().await;
    client.sync().await;
    tassert_eq!(output_order(&ds.output)[0].as_str(), "a");
    assert_coordinates_match(&manager, &ds.output)?;

    run.state
        .set_workspace_display_order(WorkspaceDisplayOrder::Sorted);
    client.sync().await;
    client.sync().await;
    assert_coordinates_match(&manager, &ds.output)?;

    // Showing another workspace destroys the empty, previously visible one.
    ds.output.show_workspace(&c);
    client.sync().await;
    client.sync().await;
    tassert_eq!(output_order(&ds.output).len(), 2);
    assert_coordinates_match(&manager, &ds.output)?;

    tassert!(!manager.missing_coordinates_at_done.get());

    Ok(())
}

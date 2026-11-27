use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::tree::ContainerSplit;
use crate::tree::Direction;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::Axis;
use std::rc::Rc;

testcase!();

/// Test that moving a tab across the axis of the titles leaves the mono
/// container even if the container's split runs along the move.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    let root = win1.tl.container_parent()?;
    root.set_split(ContainerSplit::Vertical);
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    run.cfg.create_split(ds.seat.id(), Axis::Vertical)?;
    client.sync().await;
    win2.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    let win3 = client.create_window().await?;
    win3.map2().await?;
    let win4 = client.create_window().await?;
    win4.map2().await?;
    client.sync().await;

    let tabbed = win2.tl.container_parent()?;
    tassert_ne!(tabbed.node_id(), root.node_id());
    tassert_eq!(
        tabbed.child_ids(),
        vec![
            win2.tl.server.node_id(),
            win3.tl.server.node_id(),
            win4.tl.server.node_id(),
        ],
    );
    tabbed.set_mono(Some(&*win3.tl.server));
    client.sync().await;
    tassert!(tabbed.is_mono());
    tassert_eq!(
        tabbed.node_state[LiveTL].split.get(),
        ContainerSplit::Vertical,
    );

    // Moving a tab down leaves the tabbed container, like moving the focus
    // down does, instead of reordering the tabs.
    tabbed
        .clone()
        .move_child(win3.tl.server.clone(), Direction::Down);
    client.sync().await;
    tassert_eq!(
        tabbed.child_ids(),
        vec![win2.tl.server.node_id(), win4.tl.server.node_id()],
    );
    tassert_eq!(
        root.child_ids(),
        vec![
            win1.tl.server.node_id(),
            tabbed.node_id(),
            win3.tl.server.node_id(),
        ],
    );

    Ok(())
}

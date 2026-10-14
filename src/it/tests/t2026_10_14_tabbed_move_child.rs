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

/// Test moving children horizontally into a tabbed container with a vertical
/// split.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
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
    client.sync().await;

    let root = win1.tl.container_parent()?;
    let tabbed = win2.tl.container_parent()?;
    tassert_eq!(win3.tl.container_parent()?.node_id(), tabbed.node_id());
    tabbed.set_mono(Some(&*win3.tl.server));
    client.sync().await;
    tassert!(tabbed.is_mono());
    tassert_eq!(
        tabbed.node_state[LiveTL].split.get(),
        ContainerSplit::Vertical,
    );

    // Moving right into the tabbed container inserts the child as the first
    // tab.
    root.clone()
        .move_child(win1.tl.server.clone(), Direction::Right);
    client.sync().await;
    tassert_eq!(
        tabbed.child_ids(),
        vec![
            win1.tl.server.node_id(),
            win2.tl.server.node_id(),
            win3.tl.server.node_id(),
        ],
    );

    // Wrap win2 in a nested container.
    win2.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    run.cfg.create_split(ds.seat.id(), Axis::Horizontal)?;
    client.sync().await;
    let nested = win2.tl.container_parent()?;
    tassert_ne!(nested.node_id(), tabbed.node_id());
    let win4 = client.create_window().await?;
    win4.map2().await?;
    client.sync().await;
    tassert_eq!(win4.tl.container_parent()?.node_id(), nested.node_id());

    // Moving left out of the nested container stops at the tabbed container.
    nested
        .clone()
        .move_child(win2.tl.server.clone(), Direction::Left);
    client.sync().await;
    tassert_eq!(win2.tl.container_parent()?.node_id(), tabbed.node_id());
    tassert_eq!(
        tabbed.child_ids(),
        vec![
            win1.tl.server.node_id(),
            win2.tl.server.node_id(),
            nested.node_id(),
            win3.tl.server.node_id(),
        ],
    );

    Ok(())
}

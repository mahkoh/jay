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

/// Test moving children within and between nested containers.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    // Wrap win2 in a nested vertical container.
    run.cfg.create_split(ds.seat.id(), Axis::Vertical)?;
    client.sync().await;

    let root = win1.tl.container_parent()?;
    let inner = win2.tl.container_parent()?;
    tassert_ne!(root.node_id(), inner.node_id());
    tassert_eq!(root.node_state[LiveTL].num_children.get(), 2);
    tassert_eq!(inner.node_state[LiveTL].num_children.get(), 1);
    tassert_eq!(
        inner.node_state[LiveTL].split.get(),
        ContainerSplit::Vertical,
    );

    // A third window is added to the container of the focused window.
    win2.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    let win3 = client.create_window().await?;
    win3.map2().await?;
    client.sync().await;
    tassert_eq!(win3.tl.container_parent()?.node_id(), inner.node_id());
    tassert_eq!(inner.node_state[LiveTL].num_children.get(), 2);

    // Moving a child up swaps it with its predecessor.
    inner
        .clone()
        .move_child(win3.tl.server.clone(), Direction::Up);
    client.sync().await;
    let ids = inner.child_ids();
    tassert_eq!(
        ids,
        vec![win3.tl.server.node_id(), win2.tl.server.node_id()]
    );

    // Moving a child to the left moves it out of the inner container into the
    // root container before the inner container.
    inner
        .clone()
        .move_child(win3.tl.server.clone(), Direction::Left);
    client.sync().await;
    tassert_eq!(win3.tl.container_parent()?.node_id(), root.node_id());
    tassert_eq!(
        root.child_ids(),
        vec![
            win1.tl.server.node_id(),
            win3.tl.server.node_id(),
            inner.node_id(),
        ],
    );

    // Moving the only remaining child of the inner container replaces the
    // inner container with the child.
    inner
        .clone()
        .move_child(win2.tl.server.clone(), Direction::Left);
    client.sync().await;
    tassert_eq!(win2.tl.container_parent()?.node_id(), root.node_id());
    tassert_eq!(
        root.child_ids(),
        vec![
            win1.tl.server.node_id(),
            win3.tl.server.node_id(),
            win2.tl.server.node_id(),
        ],
    );

    Ok(())
}

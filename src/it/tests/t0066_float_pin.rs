use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::ContainingNode;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test pinning a floating window and its interaction with workspaces.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    run.cfg.show_workspace(ds.seat.id(), "1")?;

    let client = run.create_client()?;

    let win = client.create_window().await?;
    win.map2().await?;
    win.set_floating(true);
    client.sync().await;

    let float = win.tl.float_parent()?;
    tassert!(float.node_visible(LiveTL));

    // An unpinned float is only visible on its workspace.
    run.cfg.show_workspace(ds.seat.id(), "2")?;
    client.sync().await;
    tassert!(!float.node_visible(LiveTL));
    run.cfg.show_workspace(ds.seat.id(), "1")?;
    client.sync().await;
    tassert!(float.node_visible(LiveTL));

    // Pinning the float keeps it visible on all workspaces.
    tassert!(!float.cnode_pinned());
    float.clone().cnode_set_pinned(true);
    client.sync().await;
    tassert!(float.cnode_pinned());
    tassert!(float.node_state[LiveTL].pinned.get());
    tassert!(win.tl.server.tl_data().pinned.get());

    run.cfg.show_workspace(ds.seat.id(), "2")?;
    client.sync().await;
    tassert!(float.node_visible(LiveTL));
    run.cfg.show_workspace(ds.seat.id(), "1")?;
    client.sync().await;
    tassert!(float.node_visible(LiveTL));

    // Unpinning restores the workspace visibility of the float.
    float.clone().cnode_set_pinned(false);
    client.sync().await;
    tassert!(!float.cnode_pinned());
    tassert!(!float.node_state[LiveTL].pinned.get());
    tassert!(!win.tl.server.tl_data().pinned.get());

    run.cfg.show_workspace(ds.seat.id(), "2")?;
    client.sync().await;
    tassert!(!float.node_visible(LiveTL));
    run.cfg.show_workspace(ds.seat.id(), "1")?;
    client.sync().await;
    tassert!(float.node_visible(LiveTL));

    // Turning off floating maps the window back into the tiling tree.
    win.set_floating(false);
    client.sync().await;
    tassert!(!win.tl.server.tl_data().parent_is_float.get());
    tassert!(win.tl.float_parent().is_err());
    win.tl.container_parent()?;
    tassert!(win.tl.server.node_visible(LiveTL));

    Ok(())
}

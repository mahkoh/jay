use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::Axis;
use std::rc::Rc;

testcase!();

/// Test that the title offsets of container children are calculated correctly.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    // A regular container has no overlay icon and the windows have no icons,
    // so all offsets are zero.
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let container = win2.tl.container_parent()?;
    tassert!(!container.tl_data().is_overlay_root_container[LiveTL].get());
    tassert_eq!(container.child_title_offsets(&*win1.tl.server)?, (0, 0, 0));
    tassert_eq!(container.child_title_offsets(&*win2.tl.server)?, (0, 0, 0));

    // Windows on an overlay are mapped as floats. Turning off floating maps
    // them into a container which is the root container of the overlay. Its
    // children reserve space for the overlay icon.
    let overlay = run.state.create_overlay_workspace("o1");
    run.state.show_workspace2(None, &ds.output, &overlay);
    let win3 = client.create_window().await?;
    win3.map2().await?;
    tassert!(win3.tl.server.tl_data().parent_is_float.get());
    win3.set_floating(false);
    client.sync().await;

    let overlay_container = win3.tl.container_parent()?;
    tassert!(overlay_container.tl_data().is_overlay_root_container[LiveTL].get());
    let th = overlay_container.node_state[LiveTL]
        .theme
        .sizes
        .title_height
        .get();
    tassert!(th > 0);
    tassert_eq!(
        overlay_container.child_title_offsets(&*win3.tl.server)?,
        (0, th, th),
    );

    // Nested containers do not reserve space for the overlay icon.
    run.cfg.create_split(ds.seat.id(), Axis::Horizontal)?;
    client.sync().await;
    let nested = win3.tl.container_parent()?;
    tassert_ne!(nested.node_id(), overlay_container.node_id());
    tassert!(!nested.tl_data().is_overlay_root_container[LiveTL].get());
    tassert_eq!(nested.child_title_offsets(&*win3.tl.server)?, (0, 0, 0));

    Ok(())
}

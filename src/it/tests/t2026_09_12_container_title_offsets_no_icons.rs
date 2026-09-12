use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::theme::sized::TITLE_HEIGHT;
use std::rc::Rc;

testcase!();

/// Test that the title offsets of container children are recalculated when the
/// title height changes while window icons are disabled.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    // Map a window as the root container of an overlay. Its children reserve
    // space for the overlay icon.
    let overlay = run.state.create_overlay_workspace("o1");
    run.state.show_workspace2(None, &ds.output, &overlay);
    let win = client.create_window().await?;
    win.map2().await?;
    tassert!(win.tl.server.tl_data().parent_is_float.get());
    win.set_floating(false);
    client.sync().await;

    let container = win.tl.container_parent()?;
    tassert!(container.tl_data().is_overlay_root_container[LiveTL].get());
    let th = container.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);
    tassert_eq!(container.child_title_offsets(&*win.tl.server)?, (0, th, th),);

    // Disabling window icons does not change the offsets.
    run.state.set_show_window_icons(false);
    client.sync().await;
    tassert_eq!(container.child_title_offsets(&*win.tl.server)?, (0, th, th),);

    // Changing the title height must update the offsets even though the window
    // icon size stays zero because window icons are disabled.
    let new_th = th + 10;
    run.cfg.set_size(TITLE_HEIGHT, new_th)?;
    client.sync().await;
    let th = container.node_state[LiveTL].theme.sizes.title_height.get();
    tassert_eq!(th, new_th);
    tassert_eq!(
        container.child_title_offsets(&*win.tl.server)?,
        (0, new_th, new_th),
    );

    Ok(())
}

use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::ContainingNode;
use crate::tree::FloatNode;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

fn offsets(f: &FloatNode) -> (i32, i32, i32, i32) {
    let offsets = &f.node_state[LiveTL].offsets;
    (
        offsets.overlay_icon.get(),
        offsets.pin_icon.get(),
        offsets.toplevel_icon.get(),
        offsets.title.get(),
    )
}

/// Test that the title offsets of floating windows are calculated correctly.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    win1.set_floating(true);
    client.sync().await;

    let float1 = win1.tl.float_parent()?;
    let th = float1.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);

    // No overlay, no pin, no icon: everything is at the left edge.
    tassert_eq!(offsets(&float1), (0, 0, 0, 0));

    // Pinning adds a slot for the pin icon in front of the window icon and
    // the title.
    float1.clone().cnode_set_pinned(true);
    client.sync().await;
    tassert_eq!(offsets(&float1), (0, 0, th, th));

    // Unpinning removes it again.
    float1.clone().cnode_set_pinned(false);
    client.sync().await;
    tassert_eq!(offsets(&float1), (0, 0, 0, 0));

    // A float on an overlay reserves a slot for the overlay icon.
    let overlay = run.state.create_overlay_workspace("o1");
    run.state.show_workspace2(None, &ds.output, &overlay);
    let win2 = client.create_window().await?;
    win2.map2().await?;
    let float2 = win2.tl.float_parent()?;
    client.sync().await;
    tassert_eq!(offsets(&float2), (0, th, th, th));

    // Pinning the overlay float adds another slot for the pin icon.
    float2.clone().cnode_set_pinned(true);
    client.sync().await;
    tassert_eq!(offsets(&float2), (0, th, 2 * th, 2 * th));

    Ok(())
}

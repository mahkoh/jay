use crate::it::test_error::TestError;
use crate::it::test_utils::test_window::TestWindow;
use crate::it::testrun::TestRun;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Basic test for overlays.
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;

    let window1 = client.create_window().await?;
    window1.map().await?;

    let o1 = run.state.create_overlay_workspace("o1");
    let o2 = run.state.create_overlay_workspace("o2");
    run.state.show_workspace2(None, &ds.output, &o1);

    let window2 = client.create_window().await?;
    window2.map().await?;

    let assert_visible = |w: &TestWindow| {
        tassert!(w.tl.server.node_visible(LiveTL));
        tassert_eq!(w.tl.server.node_output_id(), Some(ds.output.id));
        Ok(())
    };

    let assert_invisible = |w: &TestWindow| {
        tassert!(!w.tl.server.node_visible(LiveTL));
        tassert_eq!(
            w.tl.server.node_output_id(),
            Some(run.state.dummy_output_id),
        );
        Ok(())
    };

    tassert!(!window1.tl.server.tl_data().parent_is_float.get());
    assert_visible(&window1)?;

    tassert!(window2.tl.server.tl_data().parent_is_float.get());
    assert_visible(&window2)?;

    ds.output.hide_overlay();

    assert_visible(&window1)?;
    assert_invisible(&window2)?;

    run.state.show_workspace2(None, &ds.output, &o1);

    assert_visible(&window1)?;
    assert_visible(&window2)?;

    run.state.show_workspace2(None, &ds.output, &o2);

    assert_visible(&window1)?;
    assert_invisible(&window2)?;

    let window3 = client.create_window().await?;
    window3.map().await?;

    assert_visible(&window3)?;

    run.state.show_workspace2(None, &ds.output, &o1);

    assert_visible(&window1)?;
    assert_visible(&window2)?;
    assert_invisible(&window3)?;

    run.state.show_workspace2(None, &ds.output, &o2);

    assert_visible(&window1)?;
    assert_invisible(&window2)?;
    assert_visible(&window3)?;

    Ok(())
}

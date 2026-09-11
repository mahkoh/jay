use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestError;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test subsurface with already attached buffer
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.backend.install_default().await?;

    let seat = run.get_seat("default")?;

    run.state.eng.yield_now().await;

    run.cfg.show_workspace(seat.id(), "")?;

    let client = run.create_client()?;

    let parent = client.create_window().await?;
    parent.map().await?;
    parent.set_color(0, 0, 0, 255);

    let child = client.create_surface().await?;
    let buffer = client.create_single_pixel_buffer(Color::from_srgba_straight(255, 255, 255, 255));
    child.attach(buffer.id);
    let child_viewport = client.get_viewport(&child);
    child_viewport.set_source(0, 0, 1, 1);
    child_viewport.set_destination(100, 100);
    child.commit();

    let _sub = client.get_subsurface(child.id, parent.surface.id);
    parent.map().await?;

    tassert!(child.server.node_visible(LiveTL));

    Ok(())
}

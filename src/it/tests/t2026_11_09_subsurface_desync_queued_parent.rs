use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test that a commit of a desynchronized subsurface is applied after a queued parent
/// commit that contains older state of the subsurface.
async fn test(run: Rc<TestRun>) -> TestResult {
    run.backend.install_default().await?;
    let client = run.create_client()?;

    let window = client.create_window().await?;
    window.map().await?;
    client.sync().await;

    let parent = &window.surface.surface;
    let connector_id = run.backend.default_connector.id;
    let fifo = client.get_fifo(parent);

    let child = client.create_surface().await?;
    let viewport = client.get_viewport(&child);
    let subsurface = client.get_subsurface(child.id, parent.id);
    let buffer = client.create_single_pixel_buffer(Color::SOLID_BLACK);
    child.attach(buffer.id);
    viewport.set_destination(50, 50);
    child.commit();

    fifo.set_barrier();
    parent.commit();

    viewport.set_destination(100, 100);
    child.commit();

    fifo.wait_barrier();
    parent.commit();

    subsurface.set_desync();
    viewport.set_destination(200, 200);
    child.commit();
    client.sync().await;

    tassert_eq!(child.server.buffer_abs_pos[LiveTL].get().size(), (50, 50));

    run.state.latch(connector_id);
    client.sync().await;

    tassert_eq!(child.server.buffer_abs_pos[LiveTL].get().size(), (200, 200));

    Ok(())
}

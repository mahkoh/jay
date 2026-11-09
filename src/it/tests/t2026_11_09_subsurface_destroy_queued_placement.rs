use crate::ifs::wl_surface::WlSurface;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use std::rc::Rc;

testcase!();

fn count_elements(surface: &WlSurface, pending: bool) -> usize {
    let children = surface.children.borrow();
    let Some(children) = &*children else {
        return 0;
    };
    children
        .above
        .iter()
        .chain(children.below.iter())
        .filter(|e| e.pending.get() == pending)
        .count()
}

/// Test that a subsurface destroyed while a parent commit containing a placement is queued
/// does not get added back to the stacking order of the parent.
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
    viewport.set_destination(100, 100);
    child.commit();

    fifo.set_barrier();
    parent.commit();
    client.sync().await;

    tassert_eq!(count_elements(&parent.server, false), 1);
    tassert_eq!(count_elements(&parent.server, true), 0);

    subsurface.place_above(parent.id);
    fifo.wait_barrier();
    parent.commit();
    client.sync().await;

    tassert_eq!(count_elements(&parent.server, false), 1);
    tassert_eq!(count_elements(&parent.server, true), 1);

    subsurface.destroy();
    client.sync().await;

    tassert_eq!(count_elements(&parent.server, false), 0);
    tassert_eq!(count_elements(&parent.server, true), 1);

    run.state.latch(connector_id);
    client.sync().await;

    tassert_eq!(count_elements(&parent.server, false), 0);
    tassert_eq!(count_elements(&parent.server, true), 0);

    Ok(())
}

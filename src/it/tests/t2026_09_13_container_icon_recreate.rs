use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_jay_icon_surface::TestIconSurfaceFactoryExt;
use crate::it::testrun::TestRun;
use std::rc::Rc;

testcase!();

/// Test that the compositor creates exactly one icon surface when a window
/// returns to its container.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let factory = client.create_icon_surface_factory(win2.tl.core.id);
    client.sync().await;
    tassert_eq!(factory.surfaces.borrow().len(), 1);

    // Fullscreening the window replaces it by a placeholder in the container.
    run.cfg.set_fullscreen(ds.seat.id(), true)?;
    client.sync().await;
    tassert!(factory.surfaces.borrow()[0].finished.get());
    tassert_eq!(factory.surfaces.borrow().len(), 1);

    // Returning the window to the container creates exactly one new icon
    // surface.
    run.cfg.set_fullscreen(ds.seat.id(), false)?;
    client.sync().await;
    tassert_eq!(factory.surfaces.borrow().len(), 2);
    tassert!(!factory.surfaces.borrow()[1].finished.get());

    Ok(())
}

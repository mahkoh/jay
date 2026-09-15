use crate::ifs::wl_surface::WlSurface;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_jay_icon_surface::TestIconSurfaceFactoryExt;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::wire::XdgToplevelId;
use std::rc::Rc;

testcase!();

/// Creates an icon surface for `tl` and maps it by committing an empty state
/// followed by a commit with a buffer.
async fn create_icon(client: &TestClient, tl: XdgToplevelId) -> TestResult<Rc<WlSurface>> {
    let factory = client.create_icon_surface_factory(tl);
    client.sync().await;
    tassert_eq!(factory.surfaces.borrow().len(), 1);
    let icon = factory.surfaces.borrow()[0].clone();
    client.sync().await;
    let buffer = client.create_single_pixel_buffer(Color::from_srgb(255, 0, 0));
    client.send_wl_surface_attach(icon.wl_surface, buffer.id, 0, 0);
    client.send_wl_surface_commit(icon.wl_surface);
    client.sync().await;
    Ok(client.client.lookup(icon.wl_surface)?)
}

/// Test that the icon surfaces of a container follow the visibility of the
/// container.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let container = win1.tl.container_parent()?;
    let th = container.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);

    // An icon that is created while the container is visible is visible.
    let icon1 = create_icon(&client, win1.tl.core.id).await?;
    tassert_eq!(container.child_title_offsets(&*win1.tl.server)?, (0, 0, th));
    tassert!(icon1.node_visible(LiveTL));

    // Hiding the workspace hides the icon.
    run.cfg.show_workspace(ds.seat.id(), "other")?;
    client.sync().await;
    tassert!(!container.node_visible(LiveTL));
    tassert!(!icon1.node_visible(LiveTL));

    // Showing the workspace again makes the icon visible again.
    run.cfg.show_workspace(ds.seat.id(), "")?;
    client.sync().await;
    tassert!(container.node_visible(LiveTL));
    tassert!(icon1.node_visible(LiveTL));

    // The same applies to an icon that is created while the container is
    // hidden. The title bar reserves space for the icon, so the icon must
    // become visible once the container is shown again.
    run.cfg.show_workspace(ds.seat.id(), "other")?;
    client.sync().await;
    let icon2 = create_icon(&client, win2.tl.core.id).await?;
    tassert!(!icon2.node_visible(LiveTL));

    run.cfg.show_workspace(ds.seat.id(), "")?;
    client.sync().await;
    tassert_eq!(container.child_title_offsets(&*win2.tl.server)?, (0, 0, th));
    tassert!(icon2.node_visible(LiveTL));

    Ok(())
}

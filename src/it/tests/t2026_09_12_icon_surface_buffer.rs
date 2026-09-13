use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_jay_icon_surface::TestIconSurfaceFactoryExt;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::TreeTimeline::LiveTL;
use crate::wire::XdgToplevelId;
use std::rc::Rc;

testcase!();

/// Test the initial configuration sequence of an icon surface and that
/// mapping the surface does not cause the compositor to recreate it.
async fn check_sequence(
    client: &TestClient,
    tl: XdgToplevelId,
    th: i32,
    offsets: impl Fn() -> Result<(i32, i32, i32), TestError>,
) -> TestResult {
    let factory = client.create_icon_surface_factory(tl);
    client.sync().await;

    // The compositor creates exactly one icon surface.
    tassert_eq!(factory.surfaces.borrow().len(), 1);
    let icon = factory.surfaces.borrow()[0].clone();

    client.sync().await;
    tassert_eq!(icon.configure_size.get(), Some((th - 2, th - 2)));
    tassert_eq!(offsets()?, (0, 0, th));

    // The commit must not cause the compositor to create another icon
    // surface.
    tassert_eq!(factory.surfaces.borrow().len(), 1);

    // Attaching a buffer keeps the icon in the title bar.
    let buffer = client.create_single_pixel_buffer(Color::from_srgb(255, 0, 0));
    client.send_wl_surface_attach(icon.wl_surface, buffer.id, 0, 0);
    client.send_wl_surface_commit(icon.wl_surface);
    client.sync().await;
    tassert_eq!(offsets()?, (0, 0, th));
    tassert_eq!(factory.surfaces.borrow().len(), 1);

    // Destroying the factory removes the icon from the title bar.
    client.send_jay_icon_surface_factory_v1_destroy(factory.factory.get());
    client.sync().await;
    tassert!(icon.finished.get());
    tassert_eq!(offsets()?, (0, 0, 0));

    Ok(())
}

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    win2.set_floating(true);
    client.sync().await;

    // A window in a container.
    let container = win1.tl.container_parent()?;
    let th = container.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);
    tassert_eq!(container.child_title_offsets(&*win1.tl.server)?, (0, 0, 0));
    check_sequence(&client, win1.tl.core.id, th, || {
        container.child_title_offsets(&*win1.tl.server)
    })
    .await?;

    // A floating window.
    let float = win2.tl.float_parent()?;
    let th = float.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);
    tassert_eq!(float.node_state[LiveTL].offsets.toplevel_icon.get(), 0);
    check_sequence(&client, win2.tl.core.id, th, || {
        let offsets = &float.node_state[LiveTL].offsets;
        Ok((
            offsets.overlay_icon.get(),
            offsets.toplevel_icon.get(),
            offsets.title.get(),
        ))
    })
    .await?;

    Ok(())
}

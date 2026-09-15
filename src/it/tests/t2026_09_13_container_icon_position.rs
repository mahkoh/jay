use crate::ifs::wl_surface::WlSurface;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_jay_icon_surface::TestIconSurfaceFactoryExt;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::ContainerNode;
use crate::tree::ContainerSplit;
use crate::tree::NodeBase;
use crate::tree::ToplevelNode;
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

/// The position at which the icon of `tl` is drawn in the title bar of
/// `container`.
fn expected_icon_position(
    container: &Rc<ContainerNode>,
    tl: &dyn ToplevelNode,
) -> TestResult<(i32, i32)> {
    let abs = container.node_absolute_position(LiveTL);
    let title = container.child_title_rect(tl)?;
    let (_, toplevel_icon, _) = container.child_title_offsets(tl)?;
    Ok((
        abs.x1() + title.x1() + 1 + toplevel_icon,
        abs.y1() + title.y1() + 1,
    ))
}

/// Test that the icon surfaces of a container are repositioned when the layout
/// of the container changes.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let container = win1.tl.container_parent()?;

    let surface1 = create_icon(&client, win1.tl.core.id).await?;
    let surface2 = create_icon(&client, win2.tl.core.id).await?;

    let position = |s: &Rc<WlSurface>| {
        let pos = s.buffer_abs_pos[LiveTL].get();
        (pos.x1(), pos.y1())
    };

    // The icons sit in the title bars of their windows.
    tassert_eq!(
        position(&surface1),
        expected_icon_position(&container, &*win1.tl.server)?,
    );
    tassert_eq!(
        position(&surface2),
        expected_icon_position(&container, &*win2.tl.server)?,
    );

    // Changing the split moves the title bar of the second window. The icon
    // must move with it.
    container.set_split(ContainerSplit::Vertical);
    client.sync().await;
    tassert!(container.child_title_rect(&*win2.tl.server)?.y1() > 0);
    tassert_eq!(
        position(&surface1),
        expected_icon_position(&container, &*win1.tl.server)?,
    );
    tassert_eq!(
        position(&surface2),
        expected_icon_position(&container, &*win2.tl.server)?,
    );

    // Moving the output moves the container without resizing it.
    ds.output.set_position(37, 11);
    client.sync().await;
    tassert_eq!(
        position(&surface1),
        expected_icon_position(&container, &*win1.tl.server)?,
    );
    tassert_eq!(
        position(&surface2),
        expected_icon_position(&container, &*win2.tl.server)?,
    );

    // Switching to mono mode moves both title bars again.
    container.set_mono(Some(&*win1.tl.server));
    client.sync().await;
    tassert_eq!(
        position(&surface1),
        expected_icon_position(&container, &*win1.tl.server)?,
    );
    tassert_eq!(
        position(&surface2),
        expected_icon_position(&container, &*win2.tl.server)?,
    );

    Ok(())
}

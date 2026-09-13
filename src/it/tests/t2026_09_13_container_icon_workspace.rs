use crate::ifs::wl_surface::WlSurface;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_jay_icon_surface::TestIconSurfaceFactoryExt;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::NodeBase;
use crate::tree::toplevel_set_workspace;
use crate::wire::XdgToplevelId;
use jay_config::Axis;
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

/// Test that the icon surfaces of a container follow the container to a new
/// workspace.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    let ws = ds.output.create_normal_workspace("2");

    let client = run.create_client()?;
    let win = client.create_window().await?;
    win.map2().await?;
    client.sync().await;

    // Wrap the window in a nested container that can be moved on its own.
    run.cfg.create_split(ds.seat.id(), Axis::Horizontal)?;
    client.sync().await;
    let container = win.tl.container_parent()?;

    let surface = create_icon(&client, win.tl.core.id).await?;
    tassert_eq!(surface.node_workspace().map(|w| w.id), win.workspace_id());

    toplevel_set_workspace(&run.state, container.clone(), &ws);
    client.sync().await;
    tassert_eq!(win.workspace_id(), Some(ws.id));
    tassert_eq!(surface.node_workspace().map(|w| w.id), Some(ws.id));

    Ok(())
}

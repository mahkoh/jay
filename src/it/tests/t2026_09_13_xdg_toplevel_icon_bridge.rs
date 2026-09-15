use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::wire::XdgToplevelIconV1Id;
use crate::wire::XdgToplevelId;
use std::rc::Rc;

testcase!();

fn create_icon(client: &TestClient, color: Color) -> XdgToplevelIconV1Id {
    let buffer = client.create_single_pixel_buffer(color);
    let icon = client.send_xdg_toplevel_icon_manager_v1_create_icon();
    client.send_xdg_toplevel_icon_v1_add_buffer(icon, buffer.id, 1);
    icon
}

fn set_icon(client: &TestClient, tl: XdgToplevelId, icon: XdgToplevelIconV1Id) {
    client.send_xdg_toplevel_icon_manager_v1_set_icon(tl, icon);
}

/// Test that the xdg-toplevel-icon bridge keeps drawing the icon after the
/// window has left and re-entered the tree.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win = client.create_window().await?;
    win.map2().await?;
    client.sync().await;

    let icon = create_icon(&client, Color::from_srgb(255, 0, 0));
    set_icon(&client, win.tl.core.id, icon);
    client.sync().await;
    let reference = client.screenshot_qoi(false).await?;

    // Hiding and showing the workspace again does not change the icon.
    run.cfg.show_workspace(ds.seat.id(), "other")?;
    client.sync().await;
    run.cfg.show_workspace(ds.seat.id(), "")?;
    client.sync().await;
    tassert!(client.screenshot_qoi(false).await? == reference);

    // An icon that is set while the workspace is hidden is drawn once the
    // workspace is shown again.
    set_icon(&client, win.tl.core.id, XdgToplevelIconV1Id::NONE);
    client.sync().await;
    tassert!(client.screenshot_qoi(false).await? != reference);
    run.cfg.show_workspace(ds.seat.id(), "other")?;
    client.sync().await;
    set_icon(&client, win.tl.core.id, icon);
    client.sync().await;
    run.cfg.show_workspace(ds.seat.id(), "")?;
    client.sync().await;
    tassert!(client.screenshot_qoi(false).await? == reference);

    // The icon survives a fullscreen round trip, which replaces the window by
    // a placeholder in its container and back.
    run.cfg.set_fullscreen(ds.seat.id(), true)?;
    client.sync().await;
    run.cfg.set_fullscreen(ds.seat.id(), false)?;
    client.sync().await;
    tassert!(client.screenshot_qoi(false).await? == reference);

    Ok(())
}

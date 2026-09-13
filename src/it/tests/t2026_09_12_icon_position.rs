use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::ContainerSplit;
use crate::wire::XdgToplevelId;
use std::rc::Rc;

testcase!();

fn set_icon(client: &TestClient, tl: XdgToplevelId, color: Color) {
    let buffer = client.create_single_pixel_buffer(color);
    let icon = client
        .client
        .send_xdg_toplevel_icon_manager_v1_create_icon();
    client
        .client
        .send_xdg_toplevel_icon_v1_add_buffer(icon, buffer.id, 1);
    client
        .client
        .send_xdg_toplevel_icon_manager_v1_set_icon(tl, icon);
}

/// Test that window icons are drawn inside the title bar of their window.
async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    set_icon(&client, win1.tl.core.id, Color::from_srgb(255, 0, 0));
    set_icon(&client, win2.tl.core.id, Color::from_srgb(0, 255, 0));
    let container = win2.tl.container_parent()?;
    container.set_split(ContainerSplit::Vertical);
    client.sync().await;

    client.compare_screenshot("1", false).await?;

    // Floating windows draw their icon in their own title bar.
    win2.set_floating(true);
    client.sync().await;
    client.compare_screenshot("2", false).await?;

    Ok(())
}

use crate::ifs::wl_surface::icon_surface::jay_icon_surface_v1::IconSurface;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::ContainerNode;
use crate::tree::FloatNode;
use crate::tree::ToplevelNode;
use crate::tree::TreeTimeline::LiveTL;
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

fn container_icon(container: &ContainerNode, tl: &dyn ToplevelNode) -> Option<IconSurface> {
    for child in container.children.iter_valid(LiveTL) {
        if child.node.node_id() == tl.node_id() {
            return child.node_state[LiveTL].toplevel_icon.get();
        }
    }
    None
}

fn float_icon(f: &FloatNode) -> Option<IconSurface> {
    f.node_state[LiveTL].toplevel_icon.get()
}

fn float_offsets(f: &FloatNode) -> (i32, i32, i32, i32) {
    let offsets = &f.node_state[LiveTL].offsets;
    (
        offsets.overlay_icon.get(),
        offsets.pin_icon.get(),
        offsets.toplevel_icon.get(),
        offsets.title.get(),
    )
}

/// Test that disabling window icons removes the icon from the title bar and
/// that enabling them again restores it.
async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win = client.create_window().await?;
    win.map2().await?;
    client.sync().await;

    let container = win.tl.container_parent()?;
    let th = container.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);

    set_icon(&client, win.tl.core.id, Color::from_srgb(255, 0, 0));
    client.sync().await;
    tassert_eq!(container.child_title_offsets(&*win.tl.server)?, (0, 0, th));

    // Disabling window icons removes the icon.
    run.state.set_show_window_icons(false);
    client.sync().await;
    tassert_eq!(container.child_title_offsets(&*win.tl.server)?, (0, 0, 0));

    // Enabling them again restores it.
    run.state.set_show_window_icons(true);
    client.sync().await;
    tassert_eq!(container.child_title_offsets(&*win.tl.server)?, (0, 0, th));

    // Grayscale mode is applied to the icon.
    tassert_eq!(
        container_icon(&container, &*win.tl.server).map(|i| i.grayscale()),
        Some(false),
    );
    run.state.set_window_icons_grayscale(true);
    client.sync().await;
    tassert_eq!(
        container_icon(&container, &*win.tl.server).map(|i| i.grayscale()),
        Some(true),
    );
    run.state.set_window_icons_grayscale(false);
    client.sync().await;

    // The same applies to floating windows.
    win.set_floating(true);
    client.sync().await;
    let float = win.tl.float_parent()?;
    let th = float.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);
    tassert_eq!(float_offsets(&float), (0, 0, 0, th));

    run.state.set_show_window_icons(false);
    client.sync().await;
    tassert_eq!(float_offsets(&float), (0, 0, 0, 0));

    run.state.set_show_window_icons(true);
    client.sync().await;
    tassert_eq!(float_offsets(&float), (0, 0, 0, th));

    // Grayscale mode is applied to the icon as well.
    tassert_eq!(float_icon(&float).map(|i| i.grayscale()), Some(false));
    run.state.set_window_icons_grayscale(true);
    client.sync().await;
    tassert_eq!(float_icon(&float).map(|i| i.grayscale()), Some(true));
    run.state.set_window_icons_grayscale(false);
    client.sync().await;

    Ok(())
}

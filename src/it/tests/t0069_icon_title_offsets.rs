use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::ContainingNode;
use crate::tree::FloatNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::wire::XdgToplevelIconV1Id;
use std::rc::Rc;

testcase!();

fn float_offsets(f: &FloatNode) -> (i32, i32, i32, i32) {
    let offsets = &f.node_state[LiveTL].offsets;
    (
        offsets.overlay_icon.get(),
        offsets.pin_icon.get(),
        offsets.toplevel_icon.get(),
        offsets.title.get(),
    )
}

/// Test that windows with icons reserve space for the icon in the title bar.
async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let container = win2.tl.container_parent()?;
    let th = container.node_state[LiveTL].theme.sizes.title_height.get();
    tassert!(th > 0);
    tassert_eq!(container.child_title_offsets(&*win1.tl.server)?, (0, 0, 0));

    // Assigning an icon to win1 shifts its title to the right.
    let buffer = client.create_single_pixel_buffer(Color::from_srgb(255, 0, 0));
    let icon = client
        .client
        .send_xdg_toplevel_icon_manager_v1_create_icon();
    client
        .client
        .send_xdg_toplevel_icon_v1_add_buffer(icon, buffer.id, 1);
    client
        .client
        .send_xdg_toplevel_icon_manager_v1_set_icon(win1.tl.core.id, icon);
    client.sync().await;

    tassert_eq!(container.child_title_offsets(&*win1.tl.server)?, (0, 0, th));
    tassert_eq!(container.child_title_offsets(&*win2.tl.server)?, (0, 0, 0));

    // A floating window with an icon draws the icon at the left edge and the
    // title after it.
    win1.set_floating(true);
    client.sync().await;
    let float = win1.tl.float_parent()?;
    tassert_eq!(float_offsets(&float), (0, 0, 0, th));

    // Pinning moves the icon and title past the pin slot.
    float.clone().cnode_set_pinned(true);
    client.sync().await;
    tassert_eq!(float_offsets(&float), (0, 0, th, 2 * th));
    float.clone().cnode_set_pinned(false);
    client.sync().await;
    tassert_eq!(float_offsets(&float), (0, 0, 0, th));

    // Removing the icon removes the icon slot again.
    client
        .client
        .send_xdg_toplevel_icon_manager_v1_set_icon(win1.tl.core.id, XdgToplevelIconV1Id::NONE);
    client.sync().await;
    tassert_eq!(float_offsets(&float), (0, 0, 0, 0));

    Ok(())
}

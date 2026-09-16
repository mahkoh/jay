use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::ContainerChild;
use crate::tree::ContainerNode;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::linkedlist::NodeRef;
use jay_config::_private::WindowThemeKind;
use jay_config::theme::Color as ConfigColor;
use jay_config::theme::colors::FOCUSED_TITLE_BACKGROUND_COLOR;
use jay_config::theme::colors::SEPARATOR_COLOR;
use jay_config::theme::sized::TITLE_HEIGHT;
use std::rc::Rc;

testcase!();

fn child(
    c: &ContainerNode,
    win: &crate::it::test_utils::test_window::TestWindow,
) -> TestResult<NodeRef<ContainerChild>> {
    for node in c.children.iter_valid(LiveTL) {
        if node.node.node_id() == win.tl.server.node_id() {
            return Ok(node);
        }
    }
    bail!("window is not a child of the container")
}

/// Test the window theme of a tiled window.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    const W: WindowThemeKind = WindowThemeKind::ParentTheme;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    client.sync().await;
    let window1 = run.cfg.get_keyboard_window(ds.seat.id())?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;
    let window2 = run.cfg.get_keyboard_window(ds.seat.id())?;
    tassert_ne!(window1, window2);

    let container = win1.tl.container_parent()?;
    let child1 = child(&container, &win1)?;
    let child2 = child(&container, &win2)?;
    let global_bg = child1.node_state[LiveTL]
        .theme
        .colors
        .focused_title_background
        .get();
    let global_th = container.node_state[LiveTL].theme.sizes.title_height.get();
    let global_separator = container.node_state[LiveTL].theme.colors.separator.get();

    let red = ConfigColor::new(255, 0, 0);
    let green = ConfigColor::new(0, 255, 0);

    // Title colors only affect the decorations of the window itself.
    run.cfg
        .set_window_theme_color(window1, W, FOCUSED_TITLE_BACKGROUND_COLOR, Some(red))?;
    client.sync().await;
    tassert_eq!(
        child1.node_state[LiveTL]
            .theme
            .colors
            .focused_title_background
            .get(),
        Color::from(red),
    );
    tassert_eq!(
        child2.node_state[LiveTL]
            .theme
            .colors
            .focused_title_background
            .get(),
        global_bg,
    );

    // Sizes and the separator color are properties of the container and are not used
    // for tiled windows.
    run.cfg
        .set_window_theme_size(window1, W, TITLE_HEIGHT, Some(global_th + 7))?;
    run.cfg
        .set_window_theme_color(window1, W, SEPARATOR_COLOR, Some(green))?;
    client.sync().await;
    tassert_eq!(
        container.node_state[LiveTL].theme.sizes.title_height.get(),
        global_th,
    );
    tassert_eq!(
        container.node_state[LiveTL].theme.colors.separator.get(),
        global_separator,
    );

    // The stored settings take effect when the window becomes floating.
    win1.set_floating(true);
    client.sync().await;
    let float = win1.tl.float_parent()?;
    let theme = &float.node_state[LiveTL].theme;
    tassert_eq!(theme.sizes.title_height.get(), global_th + 7);
    tassert_eq!(theme.colors.separator.get(), Color::from(green));
    tassert_eq!(
        theme.colors.focused_title_background.get(),
        Color::from(red)
    );

    // The settings are kept when the window becomes tiled again.
    win1.set_floating(false);
    client.sync().await;
    let container = win1.tl.container_parent()?;
    let child1 = child(&container, &win1)?;
    tassert_eq!(
        child1.node_state[LiveTL]
            .theme
            .colors
            .focused_title_background
            .get(),
        Color::from(red),
    );

    Ok(())
}

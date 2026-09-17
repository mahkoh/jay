use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::theme::ContainerBordersSetting;
use crate::tree::ContainerChild;
use crate::tree::ContainerNode;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::linkedlist::NodeRef;
use jay_config::_private::WindowThemeKind;
use jay_config::theme::Color as ConfigColor;
use jay_config::theme::ContainerBorders;
use jay_config::theme::colors::BORDER_COLOR;
use jay_config::theme::colors::FOCUSED_BORDER_COLOR;
use jay_config::theme::colors::FOCUSED_TITLE_BACKGROUND_COLOR;
use jay_config::theme::colors::SEPARATOR_COLOR;
use jay_config::theme::sized::BORDER_WIDTH;
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

/// Test the container theme.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    const W: WindowThemeKind = WindowThemeKind::ParentTheme;
    const C: WindowThemeKind = WindowThemeKind::SelfTheme;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    client.sync().await;
    let window1 = run.cfg.get_keyboard_window(ds.seat.id())?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;
    let container_window = run.cfg.get_window_parent(window1)?;
    tassert_ne!(container_window.0, 0);

    let container = win1.tl.container_parent()?;
    let ctheme = &container.node_state[LiveTL].theme;
    let child1 = child(&container, &win1)?;
    let child2 = child(&container, &win2)?;
    let theme1 = &child1.node_state[LiveTL].theme;
    let theme2 = &child2.node_state[LiveTL].theme;

    tassert!(run.state.theme.colors.focused_border.get_opt().is_none());
    let global_th = ctheme.sizes.title_height.get();
    let global_bw = ctheme.sizes.border_width.get();
    let global_border = ctheme.colors.border.get();
    let global_bg = theme1.colors.focused_title_background.get();
    let global_font = theme1.title_font.get().to_string();
    let global_borders = ctheme.container_borders.get();
    tassert_ne!(global_borders, ContainerBordersSetting::Full);

    let red = ConfigColor::new(255, 0, 0);
    let green = ConfigColor::new(0, 255, 0);
    let blue = ConfigColor::new(0, 0, 255);

    // Container settings apply to the container.
    run.cfg
        .set_window_theme_size(container_window, C, TITLE_HEIGHT, Some(global_th + 7))?;
    run.cfg
        .set_window_theme_size(container_window, C, BORDER_WIDTH, Some(global_bw + 3))?;
    run.cfg
        .set_window_theme_color(container_window, C, BORDER_COLOR, Some(green))?;
    run.cfg
        .set_window_theme_color(container_window, C, SEPARATOR_COLOR, Some(blue))?;
    run.cfg.set_window_theme_container_borders(
        container_window,
        C,
        Some(ContainerBorders::Full),
    )?;
    client.sync().await;
    tassert_eq!(ctheme.sizes.title_height.get(), global_th + 7);
    tassert_eq!(ctheme.sizes.border_width.get(), global_bw + 3);
    tassert_eq!(ctheme.colors.border.get(), Color::from(green));
    tassert_eq!(ctheme.colors.separator.get(), Color::from(blue));
    tassert_eq!(
        ctheme.container_borders.get(),
        ContainerBordersSetting::Full
    );
    tassert_eq!(
        run.cfg
            .get_window_theme_container_borders(container_window, C)?,
        Some(ContainerBorders::Full),
    );

    // The border color is the default of the focused border color of the children.
    tassert_eq!(theme1.colors.focused_border.get(), Color::from(green));
    tassert_eq!(theme2.colors.focused_border.get(), Color::from(green));

    // Hiding titles applies to the container.
    run.cfg
        .set_window_theme_show_titles(container_window, C, Some(false))?;
    client.sync().await;
    tassert!(!ctheme.show_titles.get());
    run.cfg
        .set_window_theme_show_titles(container_window, C, None)?;
    client.sync().await;
    tassert!(ctheme.show_titles.get());

    // Child settings apply to all children.
    run.cfg.set_window_theme_color(
        container_window,
        C,
        FOCUSED_TITLE_BACKGROUND_COLOR,
        Some(blue),
    )?;
    run.cfg
        .set_window_theme_title_font(container_window, C, Some("container font 9"))?;
    client.sync().await;
    tassert_eq!(
        theme1.colors.focused_title_background.get(),
        Color::from(blue)
    );
    tassert_eq!(
        theme2.colors.focused_title_background.get(),
        Color::from(blue)
    );
    tassert_eq!(theme1.title_font.get().to_string(), "container font 9");
    tassert_eq!(theme2.title_font.get().to_string(), "container font 9");

    run.cfg
        .set_window_theme_window_icons_grayscale(container_window, C, Some(true))?;
    client.sync().await;
    tassert!(theme1.window_icons_grayscale.get());
    tassert!(theme2.window_icons_grayscale.get());

    // The window theme of a child takes priority over the container theme.
    run.cfg
        .set_window_theme_window_icons_grayscale(window1, W, Some(false))?;
    run.cfg
        .set_window_theme_color(window1, W, FOCUSED_TITLE_BACKGROUND_COLOR, Some(red))?;
    run.cfg
        .set_window_theme_color(window1, W, FOCUSED_BORDER_COLOR, Some(red))?;
    client.sync().await;
    tassert_eq!(
        theme1.colors.focused_title_background.get(),
        Color::from(red)
    );
    tassert_eq!(
        theme2.colors.focused_title_background.get(),
        Color::from(blue)
    );
    tassert_eq!(theme1.colors.focused_border.get(), Color::from(red));
    tassert_eq!(theme2.colors.focused_border.get(), Color::from(green));
    tassert!(!theme1.window_icons_grayscale.get());
    tassert!(theme2.window_icons_grayscale.get());

    // Unsetting the window theme falls back to the container theme.
    run.cfg
        .set_window_theme_color(window1, W, FOCUSED_TITLE_BACKGROUND_COLOR, None)?;
    client.sync().await;
    tassert_eq!(
        theme1.colors.focused_title_background.get(),
        Color::from(blue)
    );

    // The window theme of the container does not affect its children.
    run.cfg.set_window_theme_color(
        container_window,
        W,
        FOCUSED_TITLE_BACKGROUND_COLOR,
        Some(green),
    )?;
    client.sync().await;
    tassert_eq!(
        theme1.colors.focused_title_background.get(),
        Color::from(blue)
    );
    tassert_eq!(
        theme2.colors.focused_title_background.get(),
        Color::from(blue)
    );

    // Resetting the container theme falls back to the global theme.
    run.cfg.reset_window_theme(container_window, C)?;
    client.sync().await;
    tassert_eq!(ctheme.sizes.title_height.get(), global_th);
    tassert_eq!(ctheme.sizes.border_width.get(), global_bw);
    tassert_eq!(ctheme.colors.border.get(), global_border);
    tassert_eq!(ctheme.container_borders.get(), global_borders);
    tassert_eq!(theme2.colors.focused_title_background.get(), global_bg);
    tassert_eq!(theme2.title_font.get().to_string(), global_font);
    tassert_eq!(
        run.cfg
            .get_window_theme_container_borders(container_window, C)?,
        None,
    );

    Ok(())
}

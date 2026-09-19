use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use jay_config::_private::WindowThemeKind;
use jay_config::theme::Color;
use jay_config::theme::JcContainerBorders;
use jay_config::theme::colors::BORDER_COLOR;
use jay_config::theme::colors::FOCUSED_BORDER_COLOR;
use jay_config::theme::colors::FOCUSED_TITLE_BACKGROUND_COLOR;
use jay_config::theme::colors::SEPARATOR_COLOR;
use jay_config::theme::colors::UNFOCUSED_TITLE_BACKGROUND_COLOR;
use jay_config::theme::sized::BORDER_WIDTH;
use jay_config::theme::sized::TITLE_HEIGHT;
use std::rc::Rc;

testcase!();

/// Test the rendering of window and container themes.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    const W: WindowThemeKind = WindowThemeKind::ParentTheme;
    const C: WindowThemeKind = WindowThemeKind::SelfTheme;

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.set_color(200, 200, 200, 255);
    win1.map2().await?;
    client.sync().await;
    let window1 = run.cfg.get_keyboard_window(ds.seat.id())?;
    let win2 = client.create_window().await?;
    win2.set_color(100, 100, 100, 255);
    win2.map2().await?;
    client.sync().await;
    let window2 = run.cfg.get_keyboard_window(ds.seat.id())?;
    let container = run.cfg.get_window_parent(window1)?;

    client.compare_screenshot("1", false).await?;

    let red = Color::new(255, 0, 0);
    let green = Color::new(0, 255, 0);
    let blue = Color::new(0, 0, 255);
    let yellow = Color::new(255, 255, 0);
    let magenta = Color::new(255, 0, 255);
    let cyan = Color::new(0, 255, 255);

    // The container theme changes the container and the titles of all children. The
    // window theme of the first window changes its title.
    run.cfg
        .set_window_theme_size(container, C, TITLE_HEIGHT, Some(30))?;
    run.cfg
        .set_window_theme_size(container, C, BORDER_WIDTH, Some(10))?;
    run.cfg
        .set_window_theme_color(container, C, BORDER_COLOR, Some(blue))?;
    run.cfg
        .set_window_theme_color(container, C, SEPARATOR_COLOR, Some(yellow))?;
    run.cfg
        .set_window_theme_container_borders(container, C, Some(JcContainerBorders::Full))?;
    run.cfg
        .set_window_theme_color(container, C, FOCUSED_TITLE_BACKGROUND_COLOR, Some(magenta))?;
    run.cfg
        .set_window_theme_color(window1, W, UNFOCUSED_TITLE_BACKGROUND_COLOR, Some(green))?;
    client.sync().await;
    win1.map().await?;
    win2.map().await?;
    client.sync().await;
    client.compare_screenshot("2", false).await?;

    // A floating window uses its own sizes and colors.
    run.cfg
        .set_window_theme_size(window2, W, TITLE_HEIGHT, Some(40))?;
    run.cfg
        .set_window_theme_size(window2, W, BORDER_WIDTH, Some(8))?;
    run.cfg
        .set_window_theme_color(window2, W, BORDER_COLOR, Some(red))?;
    run.cfg
        .set_window_theme_color(window2, W, FOCUSED_BORDER_COLOR, Some(cyan))?;
    run.cfg
        .set_window_theme_color(window2, W, FOCUSED_TITLE_BACKGROUND_COLOR, Some(red))?;
    win2.set_floating(true);
    client.sync().await;
    win1.map().await?;
    win2.map().await?;
    client.sync().await;
    client.compare_screenshot("3", false).await?;

    // Resetting all themes restores the default look.
    win2.set_floating(false);
    run.cfg.reset_window_theme(window2, W)?;
    run.cfg.reset_window_theme(window1, W)?;
    client.sync().await;
    let container = run.cfg.get_window_parent(window1)?;
    run.cfg.reset_window_theme(container, C)?;
    client.sync().await;
    win1.map().await?;
    win2.map().await?;
    client.sync().await;
    client.compare_screenshot("1", false).await?;

    Ok(())
}

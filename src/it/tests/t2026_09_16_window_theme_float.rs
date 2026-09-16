use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use crate::tree::ToplevelNodeBase;
use crate::tree::ToplevelThemeType::ParentTheme;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::_private::WindowThemeKind;
use jay_config::theme::Color as ConfigColor;
use jay_config::theme::colors::BAR_BACKGROUND_COLOR;
use jay_config::theme::colors::FOCUSED_TITLE_BACKGROUND_COLOR;
use jay_config::theme::sized::BAR_HEIGHT;
use jay_config::theme::sized::BORDER_WIDTH;
use jay_config::theme::sized::TITLE_HEIGHT;
use std::rc::Rc;

testcase!();

fn same_color(a: Option<ConfigColor>, b: ConfigColor) -> bool {
    let Some(a) = a else {
        return false;
    };
    let a = a.to_f32_premultiplied();
    let b = b.to_f32_premultiplied();
    a.iter().zip(b.iter()).all(|(a, b)| (a - b).abs() < 0.001)
}

/// Test the window theme of a floating window.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win = client.create_window().await?;
    win.map2().await?;
    win.set_floating(true);
    client.sync().await;

    let window = run.cfg.get_keyboard_window(ds.seat.id())?;
    tassert_ne!(window.0, 0);
    const W: WindowThemeKind = WindowThemeKind::ParentTheme;

    let float = win.tl.float_parent()?;
    let theme = &float.node_state[LiveTL].theme;
    let global_th = theme.sizes.title_height.get();
    let global_bw = theme.sizes.border_width.get();
    let global_bg = theme.colors.focused_title_background.get();
    let global_font = theme.title_font.get().to_string();
    tassert!(global_th > 0);

    let red = ConfigColor::new(255, 0, 0);

    // Overrides without a value are reported as unset.
    tassert_eq!(
        run.cfg.get_window_theme_size(window, W, TITLE_HEIGHT)?,
        None
    );
    tassert_eq!(run.cfg.get_window_theme_show_titles(window, W)?, None);
    tassert!(
        run.cfg
            .get_window_theme_color(window, W, FOCUSED_TITLE_BACKGROUND_COLOR)?
            .is_none()
    );

    // Requests that are ignored do not store any overrides.
    let data = win.tl.server.tl_data();
    run.cfg
        .set_window_theme_color(window, W, BAR_BACKGROUND_COLOR, Some(red))?;
    run.cfg
        .set_window_theme_size(window, W, BAR_HEIGHT, Some(40))?;
    run.cfg
        .set_window_theme_size(window, W, TITLE_HEIGHT, Some(100_000))?;
    client.sync().await;
    tassert!(data.theme(ParentTheme).is_none());
    tassert_eq!(theme.sizes.title_height.get(), global_th);

    // Overrides are used by the float.
    run.cfg
        .set_window_theme_size(window, W, TITLE_HEIGHT, Some(global_th + 7))?;
    run.cfg
        .set_window_theme_size(window, W, BORDER_WIDTH, Some(global_bw + 3))?;
    run.cfg
        .set_window_theme_color(window, W, FOCUSED_TITLE_BACKGROUND_COLOR, Some(red))?;
    run.cfg
        .set_window_theme_title_font(window, W, Some("test font 12"))?;
    client.sync().await;
    tassert_eq!(theme.sizes.title_height.get(), global_th + 7);
    tassert_eq!(theme.sizes.border_width.get(), global_bw + 3);
    tassert_eq!(
        theme.colors.focused_title_background.get(),
        Color::from(red)
    );
    tassert_eq!(theme.title_font.get().to_string(), "test font 12");
    tassert_eq!(
        run.cfg.get_window_theme_size(window, W, TITLE_HEIGHT)?,
        Some(global_th + 7),
    );
    tassert!(same_color(
        run.cfg
            .get_window_theme_color(window, W, FOCUSED_TITLE_BACKGROUND_COLOR)?,
        red,
    ));

    // Overrides take priority over the global theme.
    run.cfg.set_size(TITLE_HEIGHT, global_th + 20)?;
    client.sync().await;
    tassert_eq!(theme.sizes.title_height.get(), global_th + 7);
    run.cfg.set_size(TITLE_HEIGHT, global_th)?;
    client.sync().await;

    // Hiding the titles sets the title height to 0.
    run.cfg
        .set_window_theme_show_titles(window, W, Some(false))?;
    client.sync().await;
    tassert_eq!(
        run.cfg.get_window_theme_show_titles(window, W)?,
        Some(false)
    );
    tassert_eq!(theme.sizes.title_height.get(), 0);
    run.cfg.set_window_theme_show_titles(window, W, None)?;
    client.sync().await;
    tassert_eq!(run.cfg.get_window_theme_show_titles(window, W)?, None);
    tassert_eq!(theme.sizes.title_height.get(), global_th + 7);

    // Unsetting an override falls back to the global theme.
    run.cfg
        .set_window_theme_size(window, W, TITLE_HEIGHT, None)?;
    client.sync().await;
    tassert_eq!(
        run.cfg.get_window_theme_size(window, W, TITLE_HEIGHT)?,
        None
    );
    tassert_eq!(theme.sizes.title_height.get(), global_th);

    // Sizes outside the valid range are ignored.
    run.cfg
        .set_window_theme_size(window, W, TITLE_HEIGHT, Some(100_000))?;
    client.sync().await;
    tassert_eq!(
        run.cfg.get_window_theme_size(window, W, TITLE_HEIGHT)?,
        None
    );
    tassert_eq!(theme.sizes.title_height.get(), global_th);

    // Resetting removes all overrides.
    run.cfg.reset_window_theme(window, W)?;
    client.sync().await;
    tassert_eq!(
        run.cfg.get_window_theme_size(window, W, BORDER_WIDTH)?,
        None
    );
    tassert_eq!(theme.sizes.border_width.get(), global_bw);
    tassert_eq!(theme.colors.focused_title_background.get(), global_bg);
    tassert_eq!(theme.title_font.get().to_string(), global_font);

    Ok(())
}

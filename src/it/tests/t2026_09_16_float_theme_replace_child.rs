use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::_private::WindowThemeKind;
use jay_config::Axis;
use jay_config::theme::sized::TITLE_HEIGHT;
use std::rc::Rc;

testcase!();

/// Test that the theme of a float is recomputed when its child is replaced.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win = client.create_window().await?;
    win.map2().await?;
    win.set_floating(true);
    client.sync().await;
    let window = run.cfg.get_keyboard_window(ds.seat.id())?;

    let float = win.tl.float_parent()?;
    let theme = &float.node_state[LiveTL].theme;
    let global_th = theme.sizes.title_height.get();

    run.cfg.set_window_theme_size(
        window,
        WindowThemeKind::ParentTheme,
        TITLE_HEIGHT,
        Some(global_th + 7),
    )?;
    client.sync().await;
    tassert_eq!(theme.sizes.title_height.get(), global_th + 7);

    // Creating a split replaces the child of the float with a new container that does
    // not have a theme.
    run.cfg.create_split(ds.seat.id(), Axis::Horizontal)?;
    client.sync().await;
    let container = win.tl.container_parent()?;
    let float_child = float.node_state[LiveTL].child.get();
    tassert_eq!(float_child.map(|c| c.node_id()), Some(container.node_id()),);
    tassert_eq!(theme.sizes.title_height.get(), global_th);

    Ok(())
}

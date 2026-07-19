use crate::it::test_error::TestError;
use crate::it::testrun::TestRun;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::theme::sized::BORDER_WIDTH;
use jay_config::theme::sized::TITLE_HEIGHT;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let setup = run.create_default_setup().await?;
    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;
    client.sync().await;

    // Make it floating
    run.cfg.set_floating(setup.seat.id(), true)?;
    run.sync().await;

    let float_node = run
        .state
        .root
        .stacked
        .stacked
        .iter()
        .find_map(|n| Rc::clone(&n).node_into_float())
        .unwrap();

    let pos = float_node.node_state[LiveTL].position.get();

    // 1. Huge borders: Ensure renderer doesn't crash when borders are larger than window
    let huge_bw = pos.width() / 2 + 10;
    run.cfg.set_size(BORDER_WIDTH, huge_bw)?;
    run.sync().await;
    let _ = client.take_screenshot(false).await?;

    // Reset border
    run.cfg.set_size(BORDER_WIDTH, 5)?;
    run.sync().await;

    // 2. Huge title height: Ensure renderer doesn't crash when title is larger than window
    let huge_th = pos.height() + 10;
    run.cfg.set_size(TITLE_HEIGHT, huge_th)?;
    run.cfg.set_show_titles(true)?;
    run.sync().await;
    let _ = client.take_screenshot(false).await?;

    Ok(())
}

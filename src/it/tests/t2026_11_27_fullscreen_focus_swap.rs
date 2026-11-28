use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::Direction;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Moving keyboard focus away from a fullscreen window demotes the window to a tile
/// of its workspace; focusing that tile makes the window fullscreen again.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    // Which window a map focuses is not of interest here, so the focus is set
    // explicitly before fullscreening a window.
    win1.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    let w1 = run.cfg.get_keyboard_window(ds.seat.id())?;
    win2.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    let w2 = run.cfg.get_keyboard_window(ds.seat.id())?;
    tassert_ne!(w1, w2);

    win1.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w1);

    run.cfg.set_fullscreen(ds.seat.id(), true)?;
    run.sync().await;

    let tl1 = win1.tl.server.tl_data();
    tassert!(tl1.is_fullscreen[LiveTL].get());
    tassert!(tl1.is_active_fullscreen());
    tassert!(!tl1.is_pending_fullscreen());

    // The keyboard focus is moved directly rather than with a direction, because
    // which window a direction leads to depends on the seat's fullscreen-focus-move
    // setting, and the demote happens for any focus that leaves the window.
    win2.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    run.sync().await;

    let ws = tl1.workspace[LiveTL].get().expect("no workspace");
    tassert!(tl1.is_fullscreen[LiveTL].get());
    tassert!(tl1.is_pending_fullscreen());
    tassert!(!tl1.is_active_fullscreen());
    tassert_eq!(ws.node_state[LiveTL].pending_fullscreen.get().len(), 1);
    tassert!(!win2.tl.server.tl_data().is_fullscreen[LiveTL].get());
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w2);

    win1.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    run.sync().await;

    tassert!(tl1.is_fullscreen[LiveTL].get());
    tassert!(tl1.is_active_fullscreen());
    tassert!(!tl1.is_pending_fullscreen());
    tassert_eq!(ws.node_state[LiveTL].pending_fullscreen.get().len(), 0);
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w1);
    tassert!(tl1.pending_fullscreen_node().is_none());

    Ok(())
}

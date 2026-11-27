use crate::ifs::wl_seat::FullscreenFocusMove;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::Direction;
use jay_config::input::JcFullscreenFocusMove;
use std::rc::Rc;

testcase!();

/// The fullscreen-focus-move setting reaches the seat, and keyboard navigation with
/// the `workspace` mode turns the fullscreen window into a tile of its own
/// workspace instead of moving focus to another output.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    tassert_eq!(ds.seat.fullscreen_focus_move(), FullscreenFocusMove::Output);
    run.cfg
        .set_fullscreen_focus_move(ds.seat.id(), JcFullscreenFocusMove::Workspace)?;
    run.sync().await;
    tassert_eq!(
        ds.seat.fullscreen_focus_move(),
        FullscreenFocusMove::Workspace
    );

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
        .node_do_focus(&ds.seat, crate::tree::Direction::Unspecified);
    client.sync().await;
    let w1 = run.cfg.get_keyboard_window(ds.seat.id())?;
    win2.tl
        .server
        .node_do_focus(&ds.seat, crate::tree::Direction::Unspecified);
    client.sync().await;
    let w2 = run.cfg.get_keyboard_window(ds.seat.id())?;
    tassert_ne!(w1, w2);

    win1.tl
        .server
        .node_do_focus(&ds.seat, crate::tree::Direction::Unspecified);
    client.sync().await;
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w1);

    run.cfg.set_fullscreen(ds.seat.id(), true)?;
    run.sync().await;

    let tl1 = win1.tl.server.tl_data();
    tassert!(tl1.is_active_fullscreen());

    run.cfg.focus(ds.seat.id(), Direction::Right)?;
    run.sync().await;

    tassert!(tl1.is_fullscreen[LiveTL].get());
    tassert!(tl1.is_pending_fullscreen());
    tassert!(!tl1.is_active_fullscreen());
    let ws1 = tl1.workspace[LiveTL].get().expect("no workspace");
    let ws2 = win2.tl.server.tl_data().workspace[LiveTL]
        .get()
        .expect("no workspace");
    tassert_eq!(ws1.id, ws2.id);
    tassert_eq!(ws1.node_state[LiveTL].pending_fullscreen.get().len(), 1);
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w2);

    Ok(())
}

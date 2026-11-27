use crate::ifs::wl_seat::BTN_LEFT;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::rect::Rect;
use crate::tree::ContainerNode;
use crate::tree::Direction;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Clicking the titlebar of the tile of a pending fullscreen window focuses the
/// window, which restores it to fullscreen, even though pressing a titlebar
/// normally only starts a move.
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
    tassert!(tl1.is_active_fullscreen());

    // The tile is only drawn once the fullscreen window is demoted, which any
    // focus that leaves the window does, so the focus goes to the other window.
    win2.tl
        .server
        .node_do_focus(&ds.seat, Direction::Unspecified);
    client.sync().await;
    run.sync().await;

    tassert!(tl1.is_pending_fullscreen());
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w2);
    let tile = tl1
        .pending_fullscreen_node()
        .expect("window was not made a tile");
    let container = tile
        .tl_data()
        .parent
        .get()
        .expect("tile has no parent")
        .node_into_container()
        .expect("tile parent is not a container");
    let (x, y) = tile_title_rect(&container)?.center();
    // A click outside the container, for example on the bar above it, leaves the
    // window pending, so the target is checked before the button is pressed.
    tassert!(
        container
            .node_absolute_position(LiveTL)
            .contains(x + 1, y + 1)
    );
    ds.mouse.abs(&ds.connector, x as f64 + 1.0, y as f64 + 1.0);
    run.sync().await;

    let click = ds.mouse.click(BTN_LEFT);
    run.sync().await;
    drop(click);
    run.sync().await;

    tassert!(tl1.is_fullscreen[LiveTL].get());
    tassert!(tl1.is_active_fullscreen());
    tassert!(!tl1.is_pending_fullscreen());
    tassert_eq!(run.cfg.get_keyboard_window(ds.seat.id())?, w1);

    Ok(())
}

fn tile_title_rect(c: &ContainerNode) -> TestResult<Rect> {
    let abs = c.node_absolute_position(LiveTL);
    for child in c.children.iter_valid(LiveTL) {
        if child.node.node_is_pending_fullscreen() {
            // The title bar of a container child is stored relative to the container.
            let title = child.node_state[LiveTL].title_rect.get();
            return Ok(title.move_(abs.x1(), abs.y1()));
        }
    }
    bail!("container has no pending fullscreen tile")
}

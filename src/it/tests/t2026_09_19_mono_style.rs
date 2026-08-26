use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestErrorExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::title_underline_height;
use crate::tree::ContainerMonoStyle;
use crate::tree::Direction;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use jay_config::Axis;
use jay_config::window::JcMonoStyle;
use std::rc::Rc;

testcase!();

/// Test the stacked mono style layout and scroll cycling
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client()?;
    let dss = client.get_default_seat().await?;

    let w1 = client.create_window().await?;
    w1.map2().await?;

    let seat_id = ds.seat.id();
    run.cfg.create_split(seat_id, Axis::Horizontal)?;
    run.cfg.set_mono(seat_id, true)?;
    run.cfg.set_mono_style(seat_id, JcMonoStyle::Stacked)?;

    let w2 = client.create_window().await?;
    w2.map2().await?;
    let w3 = client.create_window().await?;
    w3.map2().await?;

    // current state:
    // `[ w1 | w2 | w3 ]` stacked, with w3 visible and active

    client.sync().await;

    let container = w3.tl.container_parent()?;
    let ns = &container.node_state[LiveTL];
    let theme = &ns.theme;
    let th = theme.sizes.title_height.get();
    let tuh = title_underline_height(run.state.theme.show_titles.get());
    tassert_eq!(ns.mono_style.get(), ContainerMonoStyle::Stacked);
    tassert_eq!(ns.num_children.get(), 3);
    let width = ns.width.get();
    for (i, child) in container.children.iter_valid(LiveTL).enumerate() {
        let rect = child.node_state[LiveTL].title_rect.get();
        tassert_eq!(rect.x1(), 0);
        tassert_eq!(rect.y1(), i as i32 * (th + tuh));
        tassert_eq!(rect.width(), width);
        tassert_eq!(rect.height(), th);
    }
    let header_height = 3 * (th + tuh);
    let body = ns.mono_body.get();
    tassert_eq!(body.y1(), header_height);
    tassert_eq!(body.height(), ns.height.get() - header_height);

    let enters = dss.kb.enter.expect()?;

    let container_pos = container.tl_data().content_size.get();
    let w1_title = container.children.first().unwrap().node_state[RenderTL]
        .title_rect
        .get()
        .move_(container_pos.x1(), container_pos.y1());
    ds.mouse
        .abs(&ds.connector, w1_title.x1() as _, w1_title.y1() as _);
    client.sync().await;
    tassert!(enters.next().is_err());

    // Scrolling over the title of an inactive child cycles the visible child.
    ds.mouse.scroll(-1);
    client.sync().await;
    let enter = enters.next().with_context(|| "no enter event")?;
    tassert_eq!(enter.surface, w2.surface.id);

    ds.mouse.scroll(-1);
    client.sync().await;
    let enter = enters.next().with_context(|| "no enter event 2")?;
    tassert_eq!(enter.surface, w1.surface.id);

    // Scrolling over the separator between two titles also cycles.
    tassert!(tuh > 0);
    ds.mouse
        .abs(&ds.connector, w1_title.x1() as _, w1_title.y2() as _);
    client.sync().await;
    ds.mouse.scroll(1);
    client.sync().await;
    let enter = enters.next().with_context(|| "no enter event 3")?;
    tassert_eq!(enter.surface, w2.surface.id);

    // Moving along the horizontal axis moves the window out of the stacked
    // container.
    container
        .clone()
        .move_child(w2.tl.server.clone(), Direction::Right);
    client.sync().await;
    tassert_eq!(ns.num_children.get(), 2);
    tassert_ne!(w2.tl.container_parent()?.node_id(), container.node_id());

    Ok(())
}

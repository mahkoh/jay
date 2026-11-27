use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::rect::Rect;
use crate::theme::ContainerBorders;
use crate::theme::title_underline_height;
use crate::tree::ContainingNode;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::toplevel_set_floating;
use jay_config::_private::WindowThemeKind;
use jay_config::Axis;
use jay_config::theme::JcContainerBorders;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    let seat_id = ds.seat.id();

    let client = run.create_client()?;
    let w1 = client.create_window().await?;
    w1.map2().await?;
    let w2 = client.create_window().await?;
    w2.map2().await?;
    client.sync().await;
    let window2 = run.cfg.get_keyboard_window(seat_id)?;
    run.cfg.create_split(seat_id, Axis::Horizontal)?;
    client.sync().await;
    let container = w2.tl.container_parent()?;
    let w3 = client.create_window().await?;
    w3.map2().await?;
    client.sync().await;
    tassert_eq!(w3.tl.container_parent()?.node_id(), container.node_id());

    let container_window = run.cfg.get_window_parent(window2)?;
    run.cfg.set_window_theme_container_borders(
        container_window,
        WindowThemeKind::SelfTheme,
        Some(JcContainerBorders::Full),
    )?;
    client.sync().await;
    toplevel_set_floating(&run.state, container.clone(), true);
    client.sync().await;
    tassert!(container.tl_data().parent_is_float.get());
    tassert_eq!(container.container_borders(LiveTL), ContainerBorders::Full);

    let ns = &container.node_state[LiveTL];
    let bw = ns.theme.sizes.border_width.get();
    let tpuh =
        ns.theme.sizes.title_height.get() + title_underline_height(ns.theme.show_titles.get());
    tassert!(bw > 0);

    let de = w2.tl.server.tl_data().desired_extents.get();
    container.clone().cnode_resize_child(
        &*w2.tl.server,
        Some(de.x1() - 7),
        Some(de.y1() - 5),
        None,
        Some(de.y2() + 9),
    );
    client.sync().await;
    let de2 = w2.tl.server.tl_data().desired_extents.get();
    tassert_eq!(de2.x1(), de.x1() - 7);
    tassert_eq!(de2.y1(), de.y1() - 5);
    tassert_eq!(de2.y2(), de.y2() + 9);

    container.set_mono(Some(&*w3.tl.server));
    client.sync().await;
    let de = w3.tl.server.tl_data().desired_extents.get();
    container
        .clone()
        .cnode_set_child_position(&*w3.tl.server, de.x1() + 11, de.y1() + 13);
    client.sync().await;
    tassert_eq!(
        w3.tl.server.tl_data().desired_extents.get(),
        de.move_(11, 13),
    );

    let de = w3.tl.server.tl_data().desired_extents.get();
    let rect = Rect::new(de.x1() - 5, de.y1() - 6, de.x2() + 7, de.y2() + 8).unwrap();
    container.clone().cnode_resize_child(
        &*w3.tl.server,
        Some(rect.x1()),
        Some(rect.y1()),
        Some(rect.x2()),
        Some(rect.y2()),
    );
    client.sync().await;
    tassert_eq!(w3.tl.server.tl_data().desired_extents.get(), rect);

    container.clone().cnode_resize_child(
        &*w3.tl.server,
        None,
        None,
        Some(rect.x1() - 10),
        Some(rect.y1() - 10),
    );
    client.sync().await;
    tassert_eq!(ns.width.get(), 2 * bw);
    tassert_eq!(ns.height.get(), tpuh + 2 * bw);
    let de = w3.tl.server.tl_data().desired_extents.get();
    tassert_eq!(de.width(), 0);
    tassert_eq!(de.height(), 0);

    Ok(())
}

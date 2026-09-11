use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::rect::Rect;
use crate::tree::ContainingNode;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test the geometry of a floating window.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win = client.create_window().await?;
    win.map2().await?;
    win.set_floating(true);
    client.sync().await;

    tassert!(win.tl.server.tl_data().parent_is_float.get());
    let float = win.tl.float_parent()?;

    // A window without a remembered float size uses half the output.
    let out_pos = ds.output.node_state[LiveTL].pos.get();
    let inner_width = out_pos.width() / 2;
    let inner_height = out_pos.height() / 2;
    tassert_eq!(win.tl.core.width.get(), inner_width);
    tassert_eq!(win.tl.core.height.get(), inner_height);

    // The child occupies the area inside the border and below the title.
    let pos = float.node_state[LiveTL].position.get();
    let title = float.node_state[LiveTL].title_rect.get();
    let de = win.tl.server.tl_data().desired_extents.get();
    let border = title.x1();
    tassert!(border > 0);
    tassert_eq!(title.y1(), border);
    tassert_eq!(de.size(), (inner_width, inner_height));
    tassert_eq!(de.x1(), pos.x1() + border);
    tassert_eq!(de.x2() + border, pos.x2());
    tassert_eq!(de.y1() - border, pos.y1() + title.height() + 1);
    tassert_eq!(de.y2() + border, pos.y2());
    tassert_eq!(title.width(), pos.width() - 2 * border);

    // Moving the float moves the child by the same amount.
    float.move_(13, -7);
    client.sync().await;
    let pos2 = float.node_state[LiveTL].position.get();
    let de2 = win.tl.server.tl_data().desired_extents.get();
    tassert_eq!(pos2, pos.move_(13, -7));
    tassert_eq!(de2, de.move_(13, -7));

    // Resizing the float with child coordinates produces the requested child
    // position and size.
    let de3 = Rect::new(100, 50, 500, 350).unwrap();
    float.clone().cnode_resize_child(
        &*win.tl.server,
        Some(de3.x1()),
        Some(de3.y1()),
        Some(de3.x2()),
        Some(de3.y2()),
    );
    client.sync().await;
    tassert_eq!(win.tl.server.tl_data().desired_extents.get(), de3);
    let pos3 = float.node_state[LiveTL].position.get();
    tassert_eq!(pos3.x1(), 100 - border);
    tassert_eq!(pos3.x2(), 500 + border);
    tassert_eq!(pos3.y2(), 350 + border);

    // Setting the child position keeps the child size.
    float
        .clone()
        .cnode_set_child_position(&*win.tl.server, 200, 60);
    client.sync().await;
    tassert_eq!(
        win.tl.server.tl_data().desired_extents.get(),
        Rect::new(200, 60, 600, 360).unwrap(),
    );

    // A float that is completely outside of the output is moved back onto it.
    let pos = float.node_state[LiveTL].position.get();
    float.set_position(Rect::new_sized(10_000, 10_000, pos.width(), pos.height()).unwrap());
    client.sync().await;
    tassert!(!float.node_state[LiveTL].position.get().intersects(&out_pos));
    float.ensure_on_output(&ds.output);
    client.sync().await;
    tassert!(float.node_state[LiveTL].position.get().intersects(&out_pos));

    Ok(())
}

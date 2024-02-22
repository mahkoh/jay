use {
    crate::{
        it::{
            test_error::{TestErrorExt, TestResult},
            testrun::TestRun,
        },
        tree::ToplevelNodeBase,
    },
    jay_config::Axis,
    std::rc::Rc,
};

testcase!();

/// Test that scrolling a mono container header activates the new window
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client().await?;
    let dss = client.get_default_seat().await?;

    let w_tiled = client.create_window().await?;
    w_tiled.map2().await?;
    let w_mono1 = client.create_window().await?;
    w_mono1.map2().await?;

    run.cfg.create_split(ds.seat.id(), Axis::Horizontal)?;
    run.cfg.set_mono(ds.seat.id(), true)?;

    let w_mono2 = client.create_window().await?;
    w_mono2.map2().await?;

    // current state:
    //     | w_tiled | [ w_mono1 | w_mono2 ] | with w_mono2 visible and active

    client.sync().await;
    tassert_eq!(w_tiled.tl.width.get(), w_mono2.tl.width.get());

    let enters = dss.kb.enter.expect()?;

    ds.mouse.abs(&ds.connector, 0.0, 0.0);
    ds.mouse.abs(&ds.connector, 10.0, 500.0);
    client.sync().await;

    let enter = enters.next().with_context(|| "no enter event")?;
    tassert_eq!(enter.surface, w_tiled.surface.id);

    let mono_container = w_mono2.tl.container_parent()?;
    let container_pos = mono_container.tl_data().pos.get();
    let w_mono1_title = mono_container.render_data.borrow_mut().title_rects[0]
        .move_(container_pos.x1(), container_pos.y1());
    ds.mouse.abs(
        &ds.connector,
        w_mono1_title.x1() as _,
        w_mono1_title.y1() as _,
    );

    client.sync().await;
    tassert!(enters.next().is_err());

    ds.mouse.scroll(-1);
    client.sync().await;

    let enter = enters.next().with_context(|| "no enter event 2")?;
    tassert_eq!(enter.surface, w_mono1.surface.id);

    Ok(())
}

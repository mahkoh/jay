use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        tree::ToplevelNodeBase,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client().await?;
    let dss = client.get_default_seat().await?;

    let w_mono1 = client.create_window().await?;
    w_mono1.map2().await?;
    let w_mono2 = client.create_window().await?;
    w_mono2.map2().await?;

    run.cfg.set_mono(ds.seat.id(), true)?;

    client.sync().await;

    let container = w_mono2.tl.container_parent()?;
    let pos = container.tl_data().pos.get();
    let w_mono1_title = container.render_data.borrow_mut().title_rects[0].move_(pos.x1(), pos.y1());
    ds.mouse.abs(
        &ds.connector,
        w_mono1_title.x1() as f64,
        w_mono1_title.y1() as f64,
    );
    client.sync().await;

    let enters = dss.kb.enter.expect()?;

    ds.mouse.scroll_px(-14);
    client.sync().await;
    tassert!(enters.next().is_err());

    ds.mouse.scroll_px(-1);
    client.sync().await;
    tassert!(enters.next().is_ok());

    Ok(())
}

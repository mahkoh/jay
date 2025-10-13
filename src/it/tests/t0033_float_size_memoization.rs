use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        rect::Rect,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client1 = run.create_client().await?;
    let win1 = client1.create_window().await?;
    win1.map2().await?;

    run.cfg.set_floating(ds.seat.id(), true)?;

    client1.sync().await;
    let (w1, h1) = (win1.tl.core.width.get(), win1.tl.core.height.get());

    let float = win1.tl.float_parent()?;
    let pos = float.current.position.get();
    float
        .current
        .position
        .set(Rect::new_sized(pos.x1(), pos.x2(), pos.width() / 2, pos.height() / 2).unwrap());
    float.schedule_layout();

    client1.sync().await;
    let (w2, h2) = (win1.tl.core.width.get(), win1.tl.core.height.get());
    tassert!((w1, h1) != (w2, h2));

    run.cfg.set_floating(ds.seat.id(), false)?;

    client1.sync().await;
    let (w3, h3) = (win1.tl.core.width.get(), win1.tl.core.height.get());
    tassert!((w3, h3) != (w2, h2));

    run.cfg.set_floating(ds.seat.id(), true)?;

    client1.sync().await;
    let (w4, h4) = (win1.tl.core.width.get(), win1.tl.core.height.get());
    tassert!((w4, h4) == (w2, h2));

    Ok(())
}

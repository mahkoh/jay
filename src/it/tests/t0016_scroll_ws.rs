use {
    crate::it::{
        test_error::{TestErrorExt, TestResult},
        testrun::TestRun,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    run.cfg.show_workspace(ds.seat.id(), "1")?;

    let client = run.create_client().await?;
    let dss = client.get_default_seat().await?;

    let w1 = client.create_window().await?;
    w1.map().await?;

    run.cfg.show_workspace(ds.seat.id(), "2")?;

    let w2 = client.create_window().await?;
    w2.map().await?;

    let enters = dss.kb.enter.expect()?;

    ds.mouse.abs(&ds.connector, 0.0, 0.0);
    ds.mouse.scroll(-1);

    client.sync().await;

    let enter = enters.next().with_context(|| "no enter")?;
    tassert_eq!(enter.surface, w1.surface.id);

    Ok(())
}

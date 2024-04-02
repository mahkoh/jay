use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;

    let win1 = client.create_window().await?;
    win1.map2().await?;

    let scale = win1.surface.preferred_buffer_scale.expect()?;

    run.cfg.set_scale(&ds.output, 2.0)?;

    client.sync().await;
    tassert_eq!(scale.next()?, 2);

    run.cfg.set_scale(&ds.output, 3.0)?;

    client.sync().await;
    tassert_eq!(scale.next()?, 3);

    Ok(())
}

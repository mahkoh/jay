use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    jay_config::video::Transform,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    run.cfg
        .set_output_transform(&ds.output, Transform::FlipRotate90)?;

    let client = run.create_client().await?;
    let win = client.create_window().await?;

    let transform = win.surface.preferred_buffer_transform.expect()?;

    win.map2().await?;

    tassert_eq!(transform.next()?, 5);

    client.compare_screenshot("1", false).await?;

    Ok(())
}

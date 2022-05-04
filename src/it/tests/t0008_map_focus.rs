use {
    crate::it::{
        test_error::{TestError, TestErrorExt},
        testrun::TestRun,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client().await?;
    let default_seat = client.get_default_seat().await?;

    let enter = default_seat.kb.enter.expect()?;

    let window = client.create_window().await?;
    window.map().await?;

    let enter = enter.next().with_context(|| "Did not enter")?;
    tassert_eq!(enter.surface, window.surface.id);

    Ok(())
}

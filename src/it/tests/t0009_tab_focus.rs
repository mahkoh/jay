use {
    crate::it::{
        test_error::{TestError, TestErrorExt},
        testrun::TestRun,
    },
    jay_config::Direction,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client().await?;
    let default_seat = client.get_default_seat().await?;

    let eleave = default_seat.kb.leave.expect()?;
    let eenter = default_seat.kb.enter.expect()?;

    let window = client.create_window().await?;
    window.map().await?;

    tassert!(eenter.next().is_ok());
    tassert!(eleave.next().is_err());

    let window2 = client.create_window().await?;
    window2.map().await?;

    let leave = eleave.next().with_context(|| "Did not leave")?;
    let enter = eenter.next().with_context(|| "Did not enter")?;

    tassert_eq!(leave.surface, window.surface.id);
    tassert_eq!(enter.surface, window2.surface.id);

    eenter.none().with_context(|| "Unexpected enter")?;
    eleave.none().with_context(|| "Unexpected leave")?;

    run.cfg.focus(ds.seat.id(), Direction::Left)?;

    client.sync().await;

    let leave = eleave.next().with_context(|| "Did not leave")?;
    let enter = eenter.next().with_context(|| "Did not enter")?;

    tassert_eq!(leave.surface, window2.surface.id);
    tassert_eq!(enter.surface, window.surface.id);

    Ok(())
}

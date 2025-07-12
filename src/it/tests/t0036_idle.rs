use {
    crate::it::{
        test_error::{TestErrorExt, TestResult},
        testrun::TestRun,
    },
    std::{rc::Rc, time::Duration},
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    run.cfg.set_idle(Duration::from_micros(100))?;
    run.cfg.set_idle_grace_period(Duration::from_secs(0))?;

    let idle = ds.connector.idle.expect()?;
    tassert!(idle.next().is_err());

    run.state.wheel.timeout(3).await?;

    tassert_eq!(idle.next().with_context(|| "idle")?, true);
    tassert!(idle.next().is_err());

    ds.mouse.rel(1.0, 1.0);
    run.state.eng.yield_now().await;

    tassert_eq!(idle.next().with_context(|| "wake")?, false);

    Ok(())
}

use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestErrorExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use std::rc::Rc;
use std::time::Duration;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let default_seat = client.get_default_seat().await?;
    let enters = default_seat.kb.enter.expect()?;
    let leaves = default_seat.kb.leave.expect()?;

    let window = client.create_window().await?;
    window.map().await?;

    let enter = enters.next().with_context(|| "initial focus")?;
    tassert_eq!(enter.surface, window.surface.id);

    run.cfg.set_idle(Duration::from_micros(100))?;
    run.cfg.set_idle_grace_period(Duration::from_secs(0))?;

    let idle = ds.connector.idle.expect()?;
    tassert!(idle.next().is_err());

    run.state.wheel.timeout(3).await?;

    tassert_eq!(idle.next().with_context(|| "idle")?, true);
    tassert!(idle.next().is_err());

    client.sync().await;
    let leave = leaves.next().with_context(|| "idle focus leave")?;
    tassert_eq!(leave.surface, window.surface.id);

    ds.mouse.rel(1.0, 1.0);
    run.state.eng.yield_now().await;

    tassert_eq!(idle.next().with_context(|| "wake")?, false);

    client.sync().await;
    let enter = enters.next().with_context(|| "wake focus enter")?;
    tassert_eq!(enter.surface, window.surface.id);

    Ok(())
}

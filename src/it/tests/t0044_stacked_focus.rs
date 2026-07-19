use crate::it::test_error::TestError;
use crate::it::testrun::TestRun;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test that container focus is set to a lone stacked window when switching to its workspace
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let ds = run.create_default_setup().await?;
    let client = run.create_client().await?;

    run.cfg.show_workspace(ds.seat.id(), "1")?;
    let win1 = client.create_window().await?;
    win1.map().await?;
    client.sync().await;
    run.cfg.set_floating(ds.seat.id(), true)?;

    run.cfg.show_workspace(ds.seat.id(), "2")?;
    let win2 = client.create_window().await?;
    win2.map().await?;
    client.sync().await;

    run.cfg.show_workspace(ds.seat.id(), "1")?;

    let container = match win1.tl.server.tl_data().parent.get() {
        Some(p) => match p.node_into_float() {
            Some(p) => p,
            _ => bail!("Containing node is not a float"),
        },
        _ => bail!("Toplevel doesn't have a parent"),
    };

    tassert!(container.node_state[LiveTL].active.get());

    Ok(())
}

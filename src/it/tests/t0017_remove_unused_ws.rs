use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    run.cfg.show_workspace(ds.seat.id(), "1")?;

    tassert_eq!(run.state.workspaces.len(), 1);

    run.cfg.show_workspace(ds.seat.id(), "2")?;

    tassert_eq!(run.state.workspaces.len(), 1);

    run.cfg.show_workspace(ds.seat.id(), "1")?;

    tassert_eq!(run.state.workspaces.len(), 1);

    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.map().await?;

    run.cfg.show_workspace(ds.seat.id(), "2")?;

    tassert_eq!(run.state.workspaces.len(), 2);

    run.cfg.show_workspace(ds.seat.id(), "1")?;

    tassert_eq!(run.state.workspaces.len(), 1);

    Ok(())
}

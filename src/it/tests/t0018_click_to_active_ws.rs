use {
    crate::{
        ifs::wl_seat::BTN_LEFT,
        it::{test_error::TestResult, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    run.cfg.show_workspace(ds.seat.id(), "1")?;

    let client = run.create_client().await?;

    let win1 = client.create_window().await?;
    win1.map().await?;

    run.cfg.show_workspace(ds.seat.id(), "2")?;

    let win2 = client.create_window().await?;
    win2.map().await?;

    ds.mouse.abs(&ds.connector, 0.0, 0.0);
    ds.mouse.click(BTN_LEFT);

    client.sync().await;

    let name = ds.output.workspace.get().map(|ws| ws.name.clone());
    tassert_eq!(name.as_deref(), Some("1"));

    let pos = {
        let rd = ds.output.render_data.borrow_mut();
        rd.titles.last().map(|t| t.x1).unwrap_or(0)
    };
    ds.mouse.abs(&ds.connector, pos as _, 0.0);
    ds.mouse.click(BTN_LEFT);

    client.sync().await;

    let name = ds.output.workspace.get().map(|ws| ws.name.clone());
    tassert_eq!(name.as_deref(), Some("2"));

    Ok(())
}

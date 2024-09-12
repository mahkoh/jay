use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        tree::Node,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let win1 = client.create_window().await?;
    win1.set_color(255, 0, 0, 255);
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.set_color(0, 255, 0, 255);
    win2.map2().await?;

    client.sync().await;
    let (x, y) = win2.tl.server.node_absolute_position().center();
    ds.move_to(x, y);
    client.sync().await;
    run.cfg.set_floating(ds.seat.id(), true)?;
    client.sync().await;
    let (x, y) = win2.tl.server.node_absolute_position().center();
    ds.move_to(x, y);
    win2.map2().await?;

    let seat = client.get_default_seat().await?;
    let enter = seat.pointer.enter.expect()?;

    let region = client.comp.create_region().await?;
    win2.surface.set_input_region(&region)?;
    win2.surface.commit()?;
    client.sync().await;

    tassert_eq!(enter.next()?.surface, win1.surface.id);

    Ok(())
}

use {
    crate::{
        ifs::wl_seat::BTN_LEFT,
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
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;

    let seat = client.get_default_seat().await?;
    let button = seat.pointer.button.expect()?;

    let (x, y) = win1.tl.server.node_mapped_position().center();
    ds.move_to(x, y);
    let click = ds.mouse.click(BTN_LEFT);

    client.sync().await;
    let dev = client.data_device_manager.get_data_device(&seat.seat)?;
    let src = client.data_device_manager.create_data_source()?;
    src.set_actions(1)?;
    dev.start_drag(&src, &win1.surface, None, button.next()?.serial)?;

    client.sync().await;
    let enter = seat.pointer.enter.expect()?;

    let (x, y) = win2.tl.server.node_mapped_position().center();
    ds.move_to(x, y);

    client.sync().await;
    tassert!(enter.next().is_err());

    drop(click);

    client.sync().await;
    client.sync().await;
    tassert_eq!(enter.next()?.surface, win2.surface.id);

    Ok(())
}

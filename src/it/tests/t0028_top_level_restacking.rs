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

    let client = run.create_client().await?;

    let win1 = client.create_window().await?;
    win1.set_color(255, 0, 0, 255);
    win1.map2().await?;
    run.cfg.set_floating(ds.seat.id(), true)?;

    let win2 = client.create_window().await?;
    win2.set_color(0, 255, 0, 255);
    win2.map2().await?;
    run.cfg.set_floating(ds.seat.id(), true)?;

    {
        let parent = win1.tl.float_parent()?;
        let rect = parent.position.get();
        parent.position.set(rect.at_point(100, 100));
        parent.schedule_layout();
    }

    client.sync().await;
    client.compare_screenshot("1", false).await?;

    ds.move_to(110, 110);
    ds.mouse.click(BTN_LEFT);

    client.sync().await;
    client.compare_screenshot("2", false).await?;

    Ok(())
}

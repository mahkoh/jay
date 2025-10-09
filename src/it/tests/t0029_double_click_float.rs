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
    win1.set_color(255, 0, 0, 255);
    win1.map2().await?;
    run.cfg.set_floating(ds.seat.id(), true)?;

    for i in ["1", "2"] {
        let (x, y) = win1.tl.server.node_mapped_position().position();
        ds.move_to(x + 10, y - 3);
        ds.mouse.click(BTN_LEFT);
        ds.mouse.click(BTN_LEFT);

        client.sync().await;
        client.compare_screenshot(i, false).await?;
    }

    Ok(())
}

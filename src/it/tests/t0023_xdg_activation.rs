use {
    crate::it::{
        test_error::TestResult,
        test_utils::{
            test_ouput_node_ext::TestOutputNodeExt, test_toplevel_node_ext::TestToplevelNodeExt,
        },
        testrun::TestRun,
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

    let (x, y) = ds.output.first_toplevel()?.center();
    ds.move_to(x, y);

    client.sync().await;
    run.cfg.set_mono(ds.seat.id(), true)?;

    let token = client.activation.get_token().await?;
    client.activation.activate(&win2.surface, &token)?;
    client.sync().await;

    client.compare_screenshot("1", false).await?;

    Ok(())
}

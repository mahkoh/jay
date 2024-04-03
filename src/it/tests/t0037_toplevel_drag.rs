use {
    crate::{
        ifs::wl_seat::BTN_LEFT,
        it::{
            test_error::{TestErrorExt, TestResult},
            testrun::TestRun,
        },
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let drag_manager = client.registry.get_drag_manager().await?;
    let seat = client.get_default_seat().await?;
    let source = client.data_device_manager.create_data_source()?;
    let dev = client.data_device_manager.get_data_device(&seat.seat)?;
    let drag = drag_manager.get_xdg_toplevel_drag(&source)?;
    let win = client.create_window().await?;
    win.set_color(255, 255, 0, 255);
    win.map2().await?;

    let button = seat.pointer.button.expect()?;
    let click = ds.mouse.click(BTN_LEFT);

    client.sync().await;
    let serial = button.next().with_context(|| "button")?.serial;
    seat.pointer.set_cursor(serial, None, 0, 0)?;
    drag.attach(&win.tl, 100, 100)?;
    source.set_actions(1)?;
    dev.start_drag(&source, &win.surface, None, serial)?;

    client.sync().await;
    client.compare_screenshot("1", true).await?;
    drop(click);
    client.sync().await;
    client.compare_screenshot("2", true).await?;

    Ok(())
}

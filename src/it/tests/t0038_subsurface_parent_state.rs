use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        theme::Color,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let win = client.create_window().await?;
    win.set_color(255, 255, 255, 255);
    win.map2().await?;

    let ss = client.comp.create_surface().await?;
    let vp = client.viewporter.get_viewport(&ss)?;
    vp.set_destination(100, 100)?;
    let buf = client.spbm.create_buffer(Color::SOLID_BLACK)?;
    ss.attach(buf.id)?;
    ss.commit()?;

    let ss = client.sub.get_subsurface(ss.id, win.surface.id).await?;
    ss.set_position(0, 0)?;
    win.surface.commit()?;

    client.compare_screenshot("1", false).await?;

    ss.set_position(100, 100)?;
    win.surface.commit()?;

    client.compare_screenshot("2", false).await?;

    Ok(())
}

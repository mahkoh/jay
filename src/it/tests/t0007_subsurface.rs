use {
    crate::{
        format::ARGB8888,
        it::{test_error::TestError, testrun::TestRun},
        theme::Color,
    },
    std::rc::Rc,
};

testcase!();

/// Test subsurface positioning
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.backend.install_default()?;

    let seat = run.get_seat("default")?;

    run.state.eng.yield_now().await;

    run.cfg.show_workspace(seat.id(), "")?;

    let client = run.create_client().await?;

    let parent = client.create_window().await?;
    parent.map().await?;
    parent.set_color(0, 0, 0, 255);

    let child = client.comp.create_surface().await?;
    let sub = client
        .sub
        .get_subsurface(child.id, parent.surface.id)
        .await?;
    sub.set_position(100, 100)?;

    let pool = client.shm.create_pool(100 * 100 * 4)?;
    let buffer = pool.create_buffer(0, 100, 100, 100 * 4, ARGB8888)?;
    buffer.fill(Color::from_rgba_straight(255, 255, 255, 255));

    child.attach(buffer.id)?;

    parent.map().await?;

    seat.set_app_cursor(None);

    client.compare_screenshot("1").await?;

    sub.place_below(parent.surface.id)?;
    parent.map().await?;
    client.compare_screenshot("2").await?;

    sub.place_above(parent.surface.id)?;
    parent.map().await?;
    client.compare_screenshot("1").await?;

    Ok(())
}

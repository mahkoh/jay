use {
    crate::{
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
    let child_viewport = client.viewporter.get_viewport(&child)?;
    let sub = client
        .sub
        .get_subsurface(child.id, parent.surface.id)
        .await?;
    sub.set_position(100, 100)?;

    let buffer = client
        .spbm
        .create_buffer(Color::from_rgba_straight(255, 255, 255, 255))?;

    child.attach(buffer.id)?;
    child_viewport.set_source(0, 0, 1, 1)?;
    child_viewport.set_destination(100, 100)?;
    child.commit()?;

    parent.map().await?;

    client.compare_screenshot("1", false).await?;

    sub.place_below(parent.surface.id)?;
    child.commit()?;
    parent.map().await?;
    client.compare_screenshot("2", false).await?;

    sub.place_above(parent.surface.id)?;
    child.commit()?;
    parent.map().await?;
    client.compare_screenshot("1", false).await?;

    Ok(())
}

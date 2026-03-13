use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        theme::Color,
        tree::Node,
    },
    std::rc::Rc,
};

testcase!();

/// Test subsurface with already attached buffer
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
    let buffer = client
        .spbm
        .create_buffer(Color::from_srgba_straight(255, 255, 255, 255))?;
    child.attach(buffer.id)?;
    let child_viewport = client.viewporter.get_viewport(&child)?;
    child_viewport.set_source(0, 0, 1, 1)?;
    child_viewport.set_destination(100, 100)?;
    child.commit()?;

    let _sub = client
        .sub
        .get_subsurface(child.id, parent.surface.id)
        .await?;
    parent.map().await?;

    tassert!(child.server.node_visible());

    Ok(())
}

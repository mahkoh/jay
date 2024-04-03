use {
    crate::{
        it::{
            test_error::TestError,
            test_utils::{
                test_container_node_ext::TestContainerExt, test_ouput_node_ext::TestOutputNodeExt,
                test_toplevel_node_ext::TestToplevelNodeExt,
                test_workspace_node_ext::TestWorkspaceNodeExt,
            },
            testrun::TestRun,
        },
        theme::Color,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let seat = client.get_default_seat().await?;
    let enter = seat.pointer.enter.expect()?;
    let win1 = client.create_window().await?;
    win1.map2().await?;

    let buffer = client.spbm.create_buffer(Color::from_rgb(255, 0, 0))?;
    let surface = client.comp.create_surface().await?;
    let vp = client.viewporter.get_viewport(&surface)?;
    vp.set_destination(100, 100)?;
    surface.attach(buffer.id)?;
    surface.commit()?;

    let (x, y) = ds
        .output
        .workspace()?
        .container()?
        .first_toplevel()?
        .center();
    ds.move_to(x, y);

    client.sync().await;
    let enter = enter.next()?;
    seat.pointer
        .set_cursor(enter.serial, Some(&surface), 0, 0)?;

    client.compare_screenshot("1", true).await?;

    surface.offset(-100, -100)?;
    surface.commit()?;

    client.compare_screenshot("2", true).await?;

    Ok(())
}

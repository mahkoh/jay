use {
    crate::{
        ifs::wl_seat::wl_pointer::{IDENTICAL, INVERTED},
        it::{
            test_error::TestResult,
            test_utils::{
                test_container_node_ext::TestContainerExt, test_ouput_node_ext::TestOutputNodeExt,
                test_toplevel_node_ext::TestToplevelNodeExt,
                test_workspace_node_ext::TestWorkspaceNodeExt,
            },
            testrun::TestRun,
        },
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let win1 = client.create_window().await?;
    win1.map2().await?;

    let (x, y) = ds
        .output
        .workspace()?
        .container()?
        .first_toplevel()?
        .center();
    ds.move_to(x, y);

    let seat = client.get_default_seat().await?;
    let ard = seat.pointer.axis_relative_direction.expect()?;

    ds.mouse.scroll_px2(1, false);
    client.sync().await;
    tassert_eq!(ard.next()?.direction, IDENTICAL);

    ds.mouse.scroll_px2(1, true);
    client.sync().await;
    tassert_eq!(ard.next()?.direction, INVERTED);

    Ok(())
}

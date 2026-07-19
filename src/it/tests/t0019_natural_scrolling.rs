use crate::ifs::wl_seat::wl_pointer::IDENTICAL;
use crate::ifs::wl_seat::wl_pointer::INVERTED;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::test_utils::test_ouput_node_ext::TestOutputNodeExt;
use crate::it::test_utils::test_toplevel_node_ext::TestToplevelNodeExt;
use crate::it::test_utils::test_workspace_node_ext::TestWorkspaceNodeExt;
use crate::it::testrun::TestRun;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let win1 = client.create_window().await?;
    win1.map2().await?;

    let (x, y) = ds
        .output
        .workspace2()?
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

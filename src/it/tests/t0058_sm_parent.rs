use {
    crate::{
        ifs::xdg_session_manager_v1::REASON_LAUNCH,
        it::{test_error::TestError, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

/// No tiling restoration with parent
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let setup = run.create_session_management_setup().await?;

    let ws1 = setup.outputs[0].node.create_workspace("1");
    let ws2 = setup.outputs[1].node.create_workspace("2");

    let client = run.create_client().await?;
    let sm = client.registry.get_session_manager().await?;

    let win = client.create_window().await?;
    win.map().await?;
    tassert_eq!(win.workspace_id(), Some(ws1.id));
    win.set_workspace(&ws2);
    let session = sm.get_session(REASON_LAUNCH, None)?;
    session.add_toplevel(&win, "win")?.destroy()?;
    session.result_created().await?;
    win.tl.core.destroy()?;

    let parent = client.create_window().await?;
    let child = client.create_window_no_commit().await?;
    child.tl.core.set_parent(&parent)?;
    session.restore_toplevel(&child, "win")?;
    child.surface.surface.commit()?;
    child.tl.core.configured().await;
    child.map().await?;
    tassert_eq!(child.workspace_id(), Some(ws1.id));

    Ok(())
}

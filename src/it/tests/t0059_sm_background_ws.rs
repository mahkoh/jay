use {
    crate::{
        ifs::xdg_session_manager_v1::{REASON_LAUNCH, REASON_RECOVER},
        it::{test_error::TestError, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

/// Handling inactive workspaces
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let setup = run.create_session_management_setup().await?;

    let ws1 = setup.outputs[0].node.create_workspace("1");
    let ws2 = setup.outputs[1].node.create_workspace("2");
    let ws3 = setup.outputs[1].node.create_workspace("3");

    tassert_eq!(setup.outputs[1].node.workspace_id.get(), Some(ws2.id));

    let client = run.create_client().await?;
    let sm = client.registry.get_session_manager().await?;

    let win = client.create_window().await?;
    win.map().await?;
    tassert_eq!(win.workspace_id(), Some(ws1.id));
    win.set_workspace(&ws3);
    let session = sm.get_session(REASON_LAUNCH, None)?;
    session.add_toplevel(&win, "win")?.destroy()?;
    let session_id = session.result_created().await?;
    win.tl.core.destroy()?;

    let session = sm.get_session(REASON_LAUNCH, Some(&session_id))?;
    let (win, _) = client.restore_window(&session, "win").await?;
    win.map().await?;
    tassert_eq!(win.workspace_id(), Some(ws2.id));
    win.set_workspace(&ws3);

    let session = sm.get_session(REASON_RECOVER, Some(&session_id))?;
    let (win, _) = client.restore_window(&session, "win").await?;
    win.map().await?;
    tassert_eq!(win.workspace_id(), Some(ws3.id));
    tassert!(ws3.attention_requests.active());

    Ok(())
}

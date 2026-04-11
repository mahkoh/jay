use {
    crate::{
        ifs::xdg_session_manager_v1::REASON_LAUNCH,
        it::{test_error::TestError, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

/// Basic restoration a last-used output
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let setup = run.create_session_management_setup().await?;

    let ws1 = setup.outputs[0].node.create_workspace("1");
    let ws2 = setup.outputs[1].node.create_workspace("2");

    let client = run.create_client().await?;
    let sm = client.registry.get_session_manager().await?;
    let mut create_session = false;
    let session_id = loop {
        let win = client.create_window().await?;
        win.map().await?;
        tassert_eq!(win.workspace_id(), Some(ws1.id));
        win.set_workspace(&ws2);
        if create_session {
            let session = sm.get_session(REASON_LAUNCH, None)?;
            session.add_toplevel(&win, "win")?;
            break session.result_created().await?;
        }
        create_session = true;
    };

    let session = sm.get_session(REASON_LAUNCH, Some(&session_id))?;
    let (win, _ts) = client.restore_window(&session, "win").await?;
    win.map().await?;
    tassert_eq!(win.workspace_id(), Some(ws2.id));

    Ok(())
}

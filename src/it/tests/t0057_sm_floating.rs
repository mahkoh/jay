use {
    crate::{
        ifs::xdg_session_manager_v1::REASON_LAUNCH,
        it::{test_error::TestError, testrun::TestRun},
        tree::{Node, ToplevelNode, ToplevelNodeBase},
    },
    std::rc::Rc,
};

testcase!();

/// Floating restoration.
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
    win.set_floating(true);
    let old = win.tl.server.node_absolute_position();
    win.tl.server.tl_resize(-10, -10, -10, -10);
    run.sync().await;
    let new = win.tl.server.node_absolute_position();
    tassert_ne!(old, new);
    let session = sm.get_session(REASON_LAUNCH, None)?;
    session.add_toplevel(&win, "win")?.destroy()?;
    session.result_created().await?;
    win.tl.core.destroy()?;
    let (win, _ts) = client.restore_window(&session, "win").await?;
    win.map().await?;
    tassert_eq!(win.workspace_id(), Some(ws2.id));
    tassert!(win.tl.server.tl_data().parent_is_float.get());
    tassert_eq!(win.tl.server.node_absolute_position(), new);

    Ok(())
}

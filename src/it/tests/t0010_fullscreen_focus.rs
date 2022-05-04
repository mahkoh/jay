use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        tree::ToplevelNode,
    },
    std::rc::Rc,
};

testcase!();

/// Test that container focus is set after un-fullscreening
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);

    let client = run.create_client().await?;

    let window = client.create_window().await?;
    window.map().await?;

    tassert!(!window.tl.server.tl_data().is_fullscreen.get());

    run.cfg.set_fullscreen(ds.seat.id(), true)?;

    tassert!(window.tl.server.tl_data().is_fullscreen.get());

    run.cfg.set_fullscreen(ds.seat.id(), false)?;

    tassert!(!window.tl.server.tl_data().is_fullscreen.get());

    let container = match window.tl.server.tl_data().parent.get() {
        Some(p) => match p.node_into_container() {
            Some(p) => p,
            _ => bail!("Containing node is not a container"),
        },
        _ => bail!("Toplevel doesn't have a parent"),
    };

    tassert!(container.children.iter().next().unwrap().active.get());

    Ok(())
}

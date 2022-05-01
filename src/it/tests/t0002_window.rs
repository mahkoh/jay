use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        rect::Rect,
        tree::Node,
    },
    std::rc::Rc,
};

testcase!();

/// Create and map a single surface
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.backend.install_default();

    let client = run.create_client().await?;

    let window = client.create_window().await?;
    window.map().await?;

    tassert_eq!(window.tl.width.get(), 800);
    tassert_eq!(
        window.tl.height.get(),
        600 - 2 * (run.state.theme.title_height.get() + 1)
    );

    tassert_eq!(
        window.tl.server.node_absolute_position(),
        Rect::new_sized(
            0,
            2 * (run.state.theme.title_height.get() + 1),
            window.tl.width.get(),
            window.tl.height.get(),
        )
        .unwrap()
    );

    Ok(())
}

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
    run.backend.install_default()?;

    let client = run.create_client().await?;

    let window = client.create_window().await?;
    window.map().await?;

    tassert_eq!(window.tl.core.width.get(), 800);
    tassert_eq!(
        window.tl.core.height.get(),
        600 - 2 * run.state.theme.title_plus_underline_height()
    );

    tassert_eq!(
        window.tl.server.node_absolute_position(),
        Rect::new_sized(
            0,
            2 * run.state.theme.title_plus_underline_height(),
            window.tl.core.width.get(),
            window.tl.core.height.get(),
        )
        .unwrap()
    );

    Ok(())
}

use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        rect::Rect,
        tree::Node,
    },
    std::rc::Rc,
};

testcase!();

/// Create and map two surfaces
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.backend.install_default()?;

    let client = run.create_client().await?;

    let window = client.create_window().await?;
    window.map().await?;

    let window2 = client.create_window().await?;
    window2.map().await?;

    let otop = 2 * run.state.theme.title_plus_underline_height();
    let bw = run.state.theme.sizes.border_width.get();

    tassert_eq!(
        window.tl.server.node_absolute_position(),
        Rect::new_sized(0, otop, (800 - bw) / 2, 600 - otop).unwrap()
    );

    tassert_eq!(
        window2.tl.server.node_absolute_position(),
        Rect::new_sized((800 - bw) / 2 + bw, otop, (800 - bw) / 2, 600 - otop).unwrap()
    );

    Ok(())
}

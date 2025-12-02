use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        tree::OutputNode,
    },
    jay_config::theme::BarPosition,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let setup = run.create_default_setup().await?;

    test_bar(&run, &setup.output, 0).await?;
    test_bar(&run, &setup.output, 1).await?;
    test_bar(&run, &setup.output, 20).await?;
    test_bar(&run, &setup.output, 100).await?;

    Ok(())
}

async fn test_bar(
    run: &TestRun,
    output: &OutputNode,
    separator_width: i32,
) -> Result<(), TestError> {
    let output_rect = output.global.pos.get();

    run.cfg.set_bar_separator_width(separator_width)?;
    run.cfg.set_bar_position(BarPosition::Top)?;
    run.sync().await;

    let bar_height = run.state.theme.sizes.bar_height();
    tassert_eq!(run.state.theme.sizes.bar_separator_width(), separator_width);

    let bar_total_height = bar_height + separator_width;
    let bar_rect = output.bar_rect_with_separator.get();
    let ws_rect = output.workspace_rect.get();

    tassert_eq!(bar_rect.y1(), output_rect.y1());
    tassert_eq!(bar_rect.height(), bar_total_height);
    tassert_eq!(ws_rect.y1(), output_rect.y1() + bar_total_height);
    tassert_eq!(ws_rect.height(), output_rect.height() - bar_total_height);

    run.cfg.set_bar_position(BarPosition::Bottom)?;
    run.sync().await;

    let bar_rect = output.bar_rect_with_separator.get();
    let ws_rect = output.workspace_rect.get();
    tassert_eq!(bar_rect.y2(), output_rect.y2());
    tassert_eq!(bar_rect.height(), bar_total_height);
    tassert_eq!(ws_rect.y2(), output_rect.y2() - bar_total_height);
    tassert_eq!(ws_rect.height(), output_rect.height() - bar_total_height);

    run.cfg.set_show_bar(false)?;
    run.sync().await;

    tassert_eq!(run.cfg.get_show_bar()?, false);
    tassert_eq!(output.workspace_rect.get(), output_rect);
    tassert_eq!(output.bar_rect_with_separator.get().is_empty(), true);

    run.cfg.set_show_bar(true)?;
    run.sync().await;

    Ok(())
}

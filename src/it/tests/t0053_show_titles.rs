use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        tree::Node,
    },
    jay_config::theme::ShowTitles,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let setup = run.create_default_setup().await?;
    let output = &setup.output;
    let ws = output.workspace.get().unwrap();

    // Default is True
    tassert_eq!(run.cfg.get_show_titles_v2()?, ShowTitles::True);

    let client = run.create_client().await?;
    let win1 = client.create_window().await?;
    win1.map().await?;
    client.sync().await;

    let container = ws.container.get().unwrap();
    let theme_title_height = run.state.theme.title_height();

    // Case 1: ShowTitles::True -> Title shown for 1 window
    tassert_eq!(container.current_title_height(), theme_title_height);

    // Case 2: ShowTitles::False -> Title hidden
    run.cfg.set_show_titles_v2(ShowTitles::False)?;
    run.sync().await;
    tassert_eq!(container.current_title_height(), 0);

    // Case 3: ShowTitles::Auto
    run.cfg.set_show_titles_v2(ShowTitles::Auto)?;
    run.sync().await;

    // 1 window -> Title hidden
    tassert_eq!(container.current_title_height(), 0);

    // 2 windows -> Titles shown
    let win2 = client.create_window().await?;
    win2.map().await?;
    client.sync().await;
    tassert_eq!(container.current_title_height(), theme_title_height);

    // Floating window in Auto mode -> Title shown
    let win3 = client.create_window().await?;
    win3.map().await?;
    client.sync().await;
    run.cfg.set_floating(setup.seat.id(), true)?;
    run.sync().await;

    let float_node = run
        .state
        .root
        .stacked
        .iter()
        .find(|n| n.node_id() != container.node_id() && n.node_id() != setup.output.node_id())
        .and_then(|n| Rc::clone(&n).node_into_float())
        .unwrap();

    tassert_eq!(float_node.title_rect.get().height(), theme_title_height);

    // Test toggle: True -> False -> True
    run.cfg.set_show_titles_v2(ShowTitles::True)?;
    run.sync().await;

    run.cfg.toggle_show_titles()?;
    run.sync().await;
    tassert_eq!(run.cfg.get_show_titles_v2()?, ShowTitles::False);

    run.cfg.toggle_show_titles()?;
    run.sync().await;
    tassert_eq!(run.cfg.get_show_titles_v2()?, ShowTitles::True);

    Ok(())
}

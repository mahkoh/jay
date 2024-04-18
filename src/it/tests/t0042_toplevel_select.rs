use {
    crate::{
        ifs::wl_seat::{ToplevelSelector, BTN_LEFT},
        it::{test_error::TestResult, testrun::TestRun},
        tree::{Node, ToplevelNode},
        utils::clonecell::CloneCell,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let win1pos = win1.tl.server.node_absolute_position().position();
    let win2pos = win2.tl.server.node_absolute_position().position();
    ds.mouse.abs(
        &ds.connector,
        win1pos.0 as f64 + 2.0,
        win1pos.1 as f64 + 2.0,
    );
    run.sync().await;

    struct Selector(CloneCell<Option<Rc<dyn ToplevelNode>>>);
    impl ToplevelSelector for Rc<Selector> {
        fn set(&self, toplevel: Rc<dyn ToplevelNode>) {
            self.0.set(Some(toplevel));
        }
    }
    let selector = Rc::new(Selector(Default::default()));
    ds.seat.select_toplevel(selector.clone());

    client.compare_screenshot("1", false).await?;

    ds.mouse.abs(
        &ds.connector,
        win2pos.0 as f64 + 2.0,
        win2pos.1 as f64 + 2.0,
    );
    run.sync().await;

    client.compare_screenshot("2", false).await?;

    ds.kb.press(1);
    run.sync().await;
    tassert!(selector.0.get().is_none());

    ds.seat.select_toplevel(selector.clone());

    client.compare_screenshot("3", false).await?;

    ds.mouse.click(BTN_LEFT);

    client.compare_screenshot("4", false).await?;

    let tl = selector.0.get().expect("no toplevel selected");
    tassert_eq!(tl.node_id(), win2.tl.server.node_id);

    Ok(())
}

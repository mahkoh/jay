use {
    crate::{
        ifs::wl_surface::xdg_surface::xdg_toplevel::STATE_SUSPENDED,
        it::{
            test_error::TestResult,
            test_utils::{
                test_ouput_node_ext::TestOutputNodeExt, test_toplevel_node_ext::TestToplevelNodeExt,
            },
            testrun::TestRun,
        },
    },
    isnt::std_1::collections::IsntHashSet2Ext,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;

    let win1 = client.create_window().await?;
    win1.set_color(255, 0, 0, 255);
    win1.map2().await?;

    let win2 = client.create_window().await?;
    win2.set_color(0, 255, 0, 255);
    win2.map2().await?;

    let (x, y) = ds.output.first_toplevel()?.center();
    ds.move_to(x, y);

    tassert!(win2.tl.core.states.borrow().not_contains(&STATE_SUSPENDED));

    client.sync().await;
    run.cfg.set_mono(ds.seat.id(), true)?;

    client.sync().await;
    tassert!(win2.tl.core.states.borrow().contains(&STATE_SUSPENDED));

    run.cfg.set_mono(ds.seat.id(), false)?;

    client.sync().await;
    tassert!(win2.tl.core.states.borrow().not_contains(&STATE_SUSPENDED));

    Ok(())
}

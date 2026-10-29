use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::ContainerMonoStyle;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::Axis;
use jay_config::window::JcMonoStyle;
use std::rc::Rc;

testcase!();

/// Test that new containers use the default mono style.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win = client.create_window().await?;
    win.map2().await?;
    client.sync().await;

    let seat_id = ds.seat.id();
    run.cfg.create_split(seat_id, Axis::Horizontal)?;
    client.sync().await;
    let tabbed = win.tl.container_parent()?;
    tassert_eq!(
        tabbed.node_state[LiveTL].mono_style.get(),
        ContainerMonoStyle::Tabbed,
    );

    // Changing the default does not affect existing containers.
    run.cfg.set_default_mono_style(JcMonoStyle::Stacked)?;
    tassert_eq!(
        tabbed.node_state[LiveTL].mono_style.get(),
        ContainerMonoStyle::Tabbed,
    );

    run.cfg.create_split(seat_id, Axis::Horizontal)?;
    client.sync().await;
    let stacked = win.tl.container_parent()?;
    tassert_ne!(stacked.node_id(), tabbed.node_id());
    tassert_eq!(
        stacked.node_state[LiveTL].mono_style.get(),
        ContainerMonoStyle::Stacked,
    );

    Ok(())
}

use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::tree::ContainerMonoStyle;
use crate::tree::TreeTimeline::LiveTL;
use jay_config::Axis;
use jay_config::JcContainerTarget;
use jay_config::window::JcMonoStyle;
use std::rc::Rc;

testcase!();

/// Test setting the mono style of the target container of a seat.
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win = client.create_window().await?;
    win.map2().await?;
    client.sync().await;

    let seat_id = ds.seat.id();
    run.cfg.create_split(seat_id, Axis::Horizontal)?;
    client.sync().await;
    let container = win.tl.container_parent()?;
    let style = || container.node_state[LiveTL].mono_style.get();
    tassert_eq!(style(), ContainerMonoStyle::Tabbed);

    // The focused window is not a container, so this has no effect.
    run.cfg
        .set_container_mono_style(seat_id, JcContainerTarget::Itself, JcMonoStyle::Stacked)?;
    tassert_eq!(style(), ContainerMonoStyle::Tabbed);

    // The focused window is not a container, so this applies to its parent.
    run.cfg
        .set_container_mono_style(seat_id, JcContainerTarget::Auto, JcMonoStyle::Stacked)?;
    tassert_eq!(style(), ContainerMonoStyle::Stacked);

    run.cfg
        .set_container_mono_style(seat_id, JcContainerTarget::Parent, JcMonoStyle::Tabbed)?;
    tassert_eq!(style(), ContainerMonoStyle::Tabbed);

    Ok(())
}

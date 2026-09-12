use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::tree::NodeBase;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test the mono (tabbed) mode of a container.
async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    win1.tl.core.set_title("one");
    let win2 = client.create_window().await?;
    win2.map2().await?;
    win2.tl.core.set_title("two");
    client.sync().await;

    let container = win2.tl.container_parent()?;
    tassert!(!container.is_mono());
    tassert!(container.child_is_visible(&*win1.tl.server)?);
    tassert!(container.child_is_visible(&*win2.tl.server)?);

    // The last focused window is used when mono mode is enabled indirectly.
    container.set_own_mono(true);
    client.sync().await;
    tassert!(container.is_mono());
    let mono = container.node_state[LiveTL].mono_child.get().unwrap();
    tassert_eq!(mono.node.node_id(), win2.tl.server.node_id());
    tassert_eq!(container.tl_data().title.borrow().as_str(), "T[one, two]");
    tassert!(!container.child_is_visible(&*win1.tl.server)?);
    tassert!(container.child_is_visible(&*win2.tl.server)?);

    // In mono mode the active child fills the body of the container.
    let full = container.tl_data().content_size.get();
    let mono_body = container.node_state[LiveTL].mono_body.get();
    tassert_eq!(mono_body.x1(), 0);
    tassert_eq!(mono_body.x2(), full.width());
    tassert_eq!(mono_body.y2(), full.height());
    tassert!(mono_body.y1() > 0);
    let mono_content = container.node_state[LiveTL].mono_content.get();
    tassert_eq!(mono_content.x1(), mono_body.x1());
    tassert_eq!(mono_content.y1(), mono_body.y1());

    // Selecting a different child while already in mono mode is a no-op.
    container.set_mono(Some(&*win1.tl.server));
    client.sync().await;
    let mono = container.node_state[LiveTL].mono_child.get().unwrap();
    tassert_eq!(mono.node.node_id(), win2.tl.server.node_id());
    tassert!(!container.child_is_visible(&*win1.tl.server)?);
    tassert!(container.child_is_visible(&*win2.tl.server)?);

    // Leaving mono mode and entering it again selects the other child.
    container.set_mono(None);
    client.sync().await;
    tassert!(!container.is_mono());
    container.set_mono(Some(&*win1.tl.server));
    client.sync().await;
    let mono = container.node_state[LiveTL].mono_child.get().unwrap();
    tassert_eq!(mono.node.node_id(), win1.tl.server.node_id());
    tassert!(container.child_is_visible(&*win1.tl.server)?);
    tassert!(!container.child_is_visible(&*win2.tl.server)?);

    // Leaving mono mode makes all children visible in the split layout again.
    container.set_mono(None);
    client.sync().await;
    tassert!(!container.is_mono());
    tassert!(container.child_is_visible(&*win1.tl.server)?);
    tassert!(container.child_is_visible(&*win2.tl.server)?);
    let body1 = container.child_body(&*win1.tl.server)?;
    let body2 = container.child_body(&*win2.tl.server)?;
    tassert!(body1.x2() <= body2.x1());
    tassert_eq!(body1.y2(), full.height());

    Ok(())
}

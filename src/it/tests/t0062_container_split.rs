use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::tree::ContainerSplit;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test changing the split orientation of a container.
async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    let win2 = client.create_window().await?;
    win2.map2().await?;
    client.sync().await;

    let container = win2.tl.container_parent()?;
    let full = container.tl_data().content_size.get();
    tassert_eq!(
        container.node_state[LiveTL].split.get(),
        ContainerSplit::Horizontal,
    );

    // A horizontal split places the children next to each other and each of
    // them spans the full height of the container.
    let body1 = container.child_body(&*win1.tl.server)?;
    let body2 = container.child_body(&*win2.tl.server)?;
    tassert_eq!(body1.y1(), body2.y1());
    tassert_eq!(body1.y2(), body2.y2());
    tassert_eq!(body1.height(), body2.height());
    tassert_eq!(body1.y2(), full.height());
    tassert!(body1.x1() < body2.x1());
    tassert!(body1.x2() <= body2.x1());
    tassert_eq!(body1.x1(), 0);
    tassert_eq!(body2.x2(), full.width());

    // A vertical split places the children below each other and each of them
    // spans the full width of the container.
    container.set_split(ContainerSplit::Vertical);
    client.sync().await;
    tassert_eq!(
        container.node_state[LiveTL].split.get(),
        ContainerSplit::Vertical,
    );

    let body1 = container.child_body(&*win1.tl.server)?;
    let body2 = container.child_body(&*win2.tl.server)?;
    tassert_eq!(body1.x1(), body2.x1());
    tassert_eq!(body1.x2(), body2.x2());
    tassert_eq!(body1.width(), body2.width());
    tassert_eq!(body1.x2(), full.width());
    tassert!(body1.y1() < body2.y1());
    tassert!(body1.y2() <= body2.y1());
    tassert!(body1.y1() > 0);
    tassert_eq!(body2.y2(), full.height());

    // Switching back restores the horizontal layout.
    container.set_split(ContainerSplit::Horizontal);
    client.sync().await;
    let body1 = container.child_body(&*win1.tl.server)?;
    let body2 = container.child_body(&*win2.tl.server)?;
    tassert_eq!(body1.y2(), full.height());
    tassert!(body1.x2() <= body2.x1());
    tassert_eq!(body2.x2(), full.width());

    Ok(())
}

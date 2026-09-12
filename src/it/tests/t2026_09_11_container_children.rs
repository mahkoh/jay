use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::testrun::TestRun;
use crate::tree::ContainerChildType;
use crate::tree::ContainerSplit;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::LiveTL;
use std::rc::Rc;

testcase!();

/// Test the basic lifecycle of the children of a container.
async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;
    win1.tl.core.set_title("one");
    client.sync().await;

    let container = win1.tl.container_parent()?;
    tassert_eq!(container.node_state[LiveTL].num_children.get(), 1);
    tassert_eq!(
        container.node_state[LiveTL].split.get(),
        ContainerSplit::Horizontal,
    );
    tassert!(!container.is_mono());
    tassert_eq!(container.tl_data().title.borrow().as_str(), "H[one]");

    // A single child fills the container.
    let full = container.tl_data().content_size.get();
    let body1 = container.child_body(&*win1.tl.server)?;
    tassert_eq!(body1.x1(), 0);
    tassert_eq!(body1.x2(), full.width());
    tassert_eq!(body1.y2(), full.height());
    let title1 = container.child_title_rect(&*win1.tl.server)?;
    tassert_eq!(title1.x1(), 0);
    tassert_eq!(title1.y1(), 0);
    tassert_eq!(title1.width(), full.width());
    tassert!(title1.y2() <= body1.y1());

    // A second tiled window joins the container and splits the area equally.
    let win2 = client.create_window().await?;
    win2.map2().await?;
    win2.tl.core.set_title("two");
    client.sync().await;

    tassert_eq!(container.node_state[LiveTL].num_children.get(), 2);
    tassert_eq!(container.tl_data().title.borrow().as_str(), "H[one, two]");

    let body1 = container.child_body(&*win1.tl.server)?;
    let body2 = container.child_body(&*win2.tl.server)?;
    tassert_eq!(body1.y1(), body2.y1());
    tassert_eq!(body1.y2(), body2.y2());
    tassert_eq!(body1.x1(), 0);
    tassert_eq!(body2.x2(), full.width());
    tassert_eq!(body1.y2(), full.height());
    tassert!((body1.width() - body2.width()).abs() <= 1);
    let gap = full.width() - body1.width() - body2.width();
    tassert!(gap > 0);
    tassert_eq!(body2.x1() - body1.x2(), gap);

    // The last mapped window is active, the other one is not.
    tassert_eq!(
        container.child_ty(&*win2.tl.server)?,
        ContainerChildType::Active,
    );
    tassert_eq!(
        container.child_ty(&*win1.tl.server)?,
        ContainerChildType::Other,
    );

    // Destroying a window makes the remaining one fill the container again.
    win2.tl.core.destroy();
    client.sync().await;

    tassert_eq!(container.node_state[LiveTL].num_children.get(), 1);
    tassert_eq!(container.tl_data().title.borrow().as_str(), "H[one]");
    let body1 = container.child_body(&*win1.tl.server)?;
    tassert_eq!(body1.x2(), full.width());
    tassert_eq!(body1.y2(), full.height());

    Ok(())
}

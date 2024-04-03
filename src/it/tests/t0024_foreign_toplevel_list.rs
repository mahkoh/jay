use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        wire::WlBufferId,
    },
    ahash::AHashSet,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client1 = run.create_client().await?;
    let client2 = run.create_client().await?;

    let list = client2.registry.get_foreign_toplevel_list().await?;

    let win1 = client1.create_window().await?;
    win1.tl.core.set_title("a")?;
    win1.map().await?;
    let win2 = client1.create_window().await?;
    win2.tl.core.set_title("b")?;
    win2.map().await?;

    client2.sync().await;
    let tls = list.toplevels.take();
    tassert_eq!(tls.len(), 2);

    tassert_eq!(tls[0].title.take().as_deref(), Some("a"));
    tassert_eq!(tls[1].title.take().as_deref(), Some("b"));

    let mut ids = AHashSet::new();
    ids.insert(tls[0].identifier.take().unwrap());
    ids.insert(tls[1].identifier.take().unwrap());

    win2.tl.core.set_title("c")?;
    client1.sync().await;

    client2.sync().await;
    tassert_eq!(tls[1].title.take().as_deref(), Some("c"));

    win2.surface.attach(WlBufferId::NONE)?;
    win2.surface.commit()?;
    client1.sync().await;

    client2.sync().await;
    tassert!(tls[1].closed.get());

    win2.map().await?;

    client1.sync().await;
    let tls = list.toplevels.take();
    tassert_eq!(tls.len(), 1);
    tassert_eq!(tls[0].title.take().as_deref(), Some("c"));

    ids.insert(tls[0].identifier.take().unwrap());
    tassert_eq!(ids.len(), 3);

    Ok(())
}

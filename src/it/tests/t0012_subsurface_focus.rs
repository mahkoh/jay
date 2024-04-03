use {
    crate::{
        ifs::wl_seat::BTN_LEFT,
        it::{
            test_error::{TestErrorExt, TestResult},
            testrun::TestRun,
        },
        theme::Color,
    },
    std::rc::Rc,
};

testcase!();

/// Test that clicking on a subsurface keeps the toplevel surface focused
async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    ds.mouse.rel(1.0, 1.0);
    run.sync().await;

    let client = run.create_client().await?;
    let cds = client.get_default_seat().await?;

    let window = client.create_window().await?;
    window.map().await?;
    window.map().await?;

    let ns = client.comp.create_surface().await?;
    let nsv = client.viewporter.get_viewport(&ns)?;
    let nss = client.sub.get_subsurface(ns.id, window.surface.id).await?;
    nss.set_position(100, 100)?;
    let buffer = client.spbm.create_buffer(Color::SOLID_BLACK)?;
    ns.attach(buffer.id)?;
    nsv.set_source(0, 0, 1, 1)?;
    nsv.set_destination(100, 100)?;
    ns.commit()?;

    run.cfg.set_fullscreen(ds.seat.id(), true)?;
    client.sync().await;
    window.map().await?;

    ds.mouse.rel(-1000.0, -1000.0);

    client.sync().await;

    let motions = cds.pointer.motion.expect()?;
    let enters = cds.pointer.enter.expect()?;
    let leaves = cds.pointer.leave.expect()?;

    ds.mouse.rel(150.0, 150.0);

    client.sync().await;

    tassert!(motions.next().is_err());
    let leave = leaves.next().with_context(|| "leaves")?;
    tassert_eq!(leave.surface, window.surface.id);
    let enter = enters.next().with_context(|| "enters")?;
    tassert_eq!(enter.surface, ns.id);

    tassert!(leaves.next().is_err());
    tassert!(enters.next().is_err());

    let kenters = cds.kb.enter.expect()?;
    let kleaves = cds.kb.leave.expect()?;

    ds.mouse.click(BTN_LEFT);

    client.sync().await;

    tassert!(kleaves.next().is_err());
    tassert!(kenters.next().is_err());

    Ok(())
}

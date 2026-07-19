use crate::backend::BackendDrmDevice;
use crate::format::ARGB8888;
use crate::ifs::zwp_linux_dmabuf_feedback_v1::FB_SCANOUT;
use crate::it::test_error::TestErrorExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::video::LINEAR_MODIFIER;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup2(true).await?;

    ds.connector
        .scanout_formats
        .set(Some(Rc::new(vec![(ARGB8888, LINEAR_MODIFIER)])));
    run.state.dmabuf_feedback.update();
    run.sync().await;

    let client1 = run.create_client().await?;
    let win1 = client1.create_window().await?;
    let dmabuf = client1.registry.get_dmabuf().await?;
    let feedback = dmabuf.get_surface_feedback(&win1.surface)?;
    let feedback = feedback.feedback.expect()?;
    win1.map2().await?;

    client1.sync().await;
    let fb = feedback.last().with_context(|| "feedback 1")?;
    tassert_eq!(fb.tranches.len(), 1);
    tassert_eq!(fb.tranches[0].flags, 0);

    run.cfg.set_fullscreen(ds.seat.id(), true)?;

    client1.sync().await;
    let fb = feedback.last().with_context(|| "feedback 2")?;
    tassert_eq!(fb.tranches.len(), 2);
    tassert_eq!(
        fb.tranches[0].target_device,
        run.backend.default_drm_dev.dev_t(),
    );
    tassert_eq!(fb.tranches[0].flags, FB_SCANOUT);
    tassert_eq!(fb.tranches[1].flags, 0);

    run.cfg.set_fullscreen(ds.seat.id(), false)?;

    client1.sync().await;
    let fb = feedback.last().with_context(|| "feedback 2")?;
    tassert_eq!(fb.tranches.len(), 1);
    tassert_eq!(fb.tranches[0].flags, 0);

    Ok(())
}

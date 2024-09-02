use {
    crate::{
        ifs::zwp_linux_dmabuf_feedback_v1::SCANOUT,
        it::{
            test_error::{TestErrorExt, TestResult},
            testrun::TestRun,
        },
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup2(true).await?;

    let scanout_feedback = {
        let Some(base_fb) = run.state.drm_feedback.get() else {
            bail!("no base fb");
        };
        let Some(index) = base_fb.shared.indices.keys().copied().next() else {
            bail!("no formats");
        };
        let fb = base_fb
            .for_scanout(&run.state.drm_feedback_ids, 1234, &[index])
            .unwrap()
            .unwrap();
        Rc::new(fb)
    };

    ds.connector.feedback.set(Some(scanout_feedback.clone()));

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
        scanout_feedback.tranches[0].device
    );
    tassert_eq!(fb.tranches[0].flags, SCANOUT);
    tassert_eq!(fb.tranches[1].flags, 0);

    run.cfg.set_fullscreen(ds.seat.id(), false)?;

    client1.sync().await;
    let fb = feedback.last().with_context(|| "feedback 2")?;
    tassert_eq!(fb.tranches.len(), 1);
    tassert_eq!(fb.tranches[0].flags, 0);

    Ok(())
}

use crate::backend::BackendDrmDevice;
use crate::backend::BackendEvent;
use crate::it::test_error::TestError;
use crate::it::testrun::TestRun;
use jay_config::video::DrmDevice;
use std::rc::Rc;

testcase!();

/// Test that flip-margin auto-adjustment can be read and changed.
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.state.backend_events.push(BackendEvent::NewDrmDevice(
        run.backend.default_drm_dev.clone(),
    ));
    run.sync().await;

    let backend_device = &run.backend.default_drm_dev;
    let device = DrmDevice(backend_device.id().raw() as _);

    tassert!(run.cfg.flip_margin_auto_adjustment_enabled(device)?);

    run.cfg
        .set_flip_margin_auto_adjustment_enabled(device, false)?;
    tassert!(!backend_device.flip_margin_auto_adjustment_enabled());
    tassert!(!run.cfg.flip_margin_auto_adjustment_enabled(device)?);

    run.cfg
        .set_flip_margin_auto_adjustment_enabled(device, true)?;
    tassert!(backend_device.flip_margin_auto_adjustment_enabled());
    tassert!(run.cfg.flip_margin_auto_adjustment_enabled(device)?);

    Ok(())
}

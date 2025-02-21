use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        theme::Color,
        utils::errorfmt::ErrorFmt,
        video::drm::{DrmError, sync_obj::SyncObjPoint, wait_for_sync_obj::SyncObjWaiter},
    },
    std::{cell::Cell, rc::Rc},
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup2(true).await?;

    struct Waiter(Cell<bool>);
    impl SyncObjWaiter for Waiter {
        fn done(self: Rc<Self>, result: Result<(), DrmError>) {
            result.unwrap();
            self.0.set(true);
        }
    }
    let waiter = Rc::new(Waiter(Cell::new(false)));

    let eng = run.state.render_ctx.get().unwrap();
    let Some(ctx) = eng.sync_obj_ctx() else {
        log::warn!(
            "Cannot test explicit sync on this system: render context does not support sync objects"
        );
        return Ok(());
    };
    let syncobj = match ctx.create_sync_obj() {
        Ok(s) => Rc::new(s),
        Err(e) => {
            log::warn!("Cannot test explicit sync on this system: {}", ErrorFmt(e));
            return Ok(());
        }
    };
    let _wait_handle =
        run.state
            .wait_for_sync_obj
            .wait(&syncobj, SyncObjPoint(2), true, waiter.clone())?;

    let client = run.create_client().await?;

    let buf1 = client.spbm.create_buffer(Color::SOLID_BLACK)?;
    let buf2 = client.spbm.create_buffer(Color::SOLID_BLACK)?;

    let syncobj_manager = client.registry.get_syncobj_manager().await?;
    let timeline = syncobj_manager.import_timeline(&syncobj)?;

    let win = client.create_window().await?;
    let sync = syncobj_manager.get_surface(&win.surface)?;
    win.surface.attach(buf1.id)?;
    sync.set_acquire_point(&timeline, 1)?;
    sync.set_release_point(&timeline, 2)?;
    win.surface.commit()?;
    sync.destroy()?;
    win.surface.attach(buf2.id)?;
    win.surface.commit()?;

    client.sync().await;
    tassert_eq!(waiter.0.get(), false);

    ctx.signal(&syncobj, SyncObjPoint(1))?;

    client.sync().await;
    tassert_eq!(waiter.0.get(), true);

    Ok(())
}

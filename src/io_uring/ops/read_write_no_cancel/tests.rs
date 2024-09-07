use {
    crate::{
        async_engine::AsyncEngine,
        io_uring::{IoUring, IoUringError},
        utils::{oserror::OsError, queue::AsyncQueue},
        wheel::Wheel,
    },
    std::rc::Rc,
    uapi::c::ECANCELED,
};

fn cancel(timeout: bool) {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let ring2 = ring.clone();
    let ring3 = ring.clone();
    let wheel = Wheel::new(&eng, &ring).unwrap();
    let queue = Rc::new(AsyncQueue::new());
    let queue2 = queue.clone();
    let _fut1 = eng.spawn(async move {
        let (read, _write) = uapi::pipe().unwrap();
        let mut buf = [10];
        let res = ring
            .read_no_cancel(read.borrow(), !0, &mut buf, |id| queue.push(id))
            .await;
        assert!(matches!(
            res.unwrap_err(),
            IoUringError::OsError(OsError(ECANCELED))
        ));
        ring.stop();
    });
    let _fut2 = eng.spawn(async move {
        let id = queue2.pop().await;
        if timeout {
            wheel.timeout(1).await.unwrap();
        }
        ring2.cancel(id);
    });
    ring3.run().unwrap();
}

#[test]
fn cancel_in_kernel() {
    cancel(true);
}

#[test]
fn cancel_in_userspace() {
    cancel(true);
}

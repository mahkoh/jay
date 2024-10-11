use {
    crate::{
        io_uring::IoUring,
        utils::{
            asyncevent::AsyncEvent, errorfmt::ErrorFmt, event_listener::EventSource, timer::TimerFd,
        },
    },
    std::{rc::Rc, time::Duration},
    uapi::c,
};

pub async fn run_const_clock<T, F>(
    duration: Duration,
    ring: &Rc<IoUring>,
    source: &EventSource<T>,
    mut f: F,
) where
    T: ?Sized,
    F: FnMut(Rc<T>),
{
    let timer = match TimerFd::new(c::CLOCK_MONOTONIC) {
        Ok(fd) => fd,
        Err(e) => {
            log::error!("Could not create timerfd: {}", ErrorFmt(e));
            return;
        }
    };
    if let Err(e) = timer.program(Some(duration), Some(duration)) {
        log::error!("Could not program timerfd: {}", ErrorFmt(e));
        return;
    }
    let ae = Rc::new(AsyncEvent::default());
    loop {
        if let Err(e) = timer.expired(&ring).await {
            log::error!("Could not wait for timerfd to expire: {}", ErrorFmt(e));
            return;
        }
        let mut dispatched_any = false;
        for el in source.iter() {
            dispatched_any = true;
            f(el);
        }
        if !dispatched_any {
            let ae2 = ae.clone();
            source.on_attach(Box::new(move || ae2.trigger()));
            ae.triggered().await;
        }
    }
}

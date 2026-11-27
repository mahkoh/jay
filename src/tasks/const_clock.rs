use crate::state::State;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::event_listener::EventSource;
use crate::utils::timer::next_expiration;
use az::saturating_cast;
use std::rc::Rc;
use std::time::Duration;

pub async fn run_const_clock<T, F>(
    duration: Duration,
    state: &Rc<State>,
    source: &EventSource<T>,
    mut f: F,
) where
    T: ?Sized,
    F: FnMut(Rc<T>),
{
    let interval = saturating_cast(duration.as_nanos());
    let mut next = state.now_nsec().saturating_add(interval);
    let ae = Rc::new(AsyncEvent::default());
    loop {
        if let Err(e) = state.ring.timeout(next).await {
            log::error!("Could not wait for timeout to expire: {}", ErrorFmt(e));
            return;
        }
        let mut dispatched_any = false;
        source.for_each(|el| {
            dispatched_any = true;
            f(el);
        });
        if !dispatched_any {
            let ae2 = ae.clone();
            source.on_attach(Box::new(move || ae2.trigger()));
            ae.triggered().await;
        }
        next = next_expiration(state.now_nsec(), next, interval);
    }
}

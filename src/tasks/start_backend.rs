use {
    crate::{
        backends::{metal, x::XBackend},
        state::State,
        utils::errorfmt::ErrorFmt,
    },
    std::{future::pending, rc::Rc},
};

pub async fn start_backend(state: Rc<State>) {
    log::info!("Trying to start X backend");
    // let e = match XorgBackend::new(&state) {
    //     Ok(b) => {
    //         state.backend.set(Some(b));
    //         pending().await
    //     }
    //     Err(e) => e,
    // };
    let e = match XBackend::run(&state).await {
        Ok(_) => pending().await,
        Err(e) => e,
    };
    log::warn!("Could not start X backend: {}", ErrorFmt(e));
    log::info!("Trying to start metal backend");
    let e = metal::run(state.clone()).await;
    log::error!("Metal backend failed: {}", ErrorFmt(e));
    log::warn!("Shutting down");
    state.el.stop();
}

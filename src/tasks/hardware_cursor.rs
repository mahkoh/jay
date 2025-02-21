use {
    crate::{state::State, utils::errorfmt::ErrorFmt},
    futures_util::{FutureExt, select},
    std::rc::Rc,
};

pub async fn handle_hardware_cursor_tick(state: Rc<State>) {
    loop {
        let cursor = match state.hardware_tick_cursor.pop().await {
            Some(c) => c,
            _ => continue,
        };
        if !cursor.needs_tick() {
            continue;
        }
        loop {
            let tick = (cursor.time_until_tick().as_nanos() + 999_999) / 1_000_000;
            if tick > 0 {
                let res = select! {
                    _ = state.hardware_tick_cursor.non_empty().fuse() => break,
                    res = state.wheel.timeout(tick as _).fuse() => res,
                };
                if let Err(e) = res {
                    log::error!("Could not wait for cursor tick: {}", ErrorFmt(e));
                    break;
                }
            } else {
                if state.hardware_tick_cursor.is_not_empty() {
                    break;
                }
            }
            cursor.tick();
            state.damage_hardware_cursors(true);
        }
    }
}

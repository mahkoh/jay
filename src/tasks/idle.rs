use {
    crate::{
        async_engine::{AsyncError, Timer},
        backend::Backend,
        state::State,
        utils::errorfmt::ErrorFmt,
    },
    futures_util::{select, FutureExt},
    std::{rc::Rc, time::Duration},
    uapi::c,
};

pub async fn idle(state: Rc<State>, backend: Rc<dyn Backend>) {
    if !backend.supports_idle() {
        return;
    }
    let timer = match state.eng.timer(c::CLOCK_MONOTONIC) {
        Ok(t) => t,
        Err(e) => {
            log::error!("Could not create idle timer: {}", ErrorFmt(e));
            return;
        }
    };
    state.idle.change.trigger();
    state.idle.timeout_changed.set(true);
    let mut idle = Idle {
        state,
        backend,
        timer,
        idle: false,
        dead: false,
        is_inhibited: false,
        last_input: now(),
    };
    idle.run().await;
}

struct Idle {
    state: Rc<State>,
    backend: Rc<dyn Backend>,
    timer: Timer,
    idle: bool,
    dead: bool,
    is_inhibited: bool,
    last_input: c::timespec,
}

impl Idle {
    async fn run(&mut self) {
        while !self.dead {
            select! {
                res = self.timer.expired().fuse() => self.handle_expired(res),
                _ = self.state.idle.change.triggered().fuse() => self.handle_idle_changes(),
            }
        }
        log::error!("Due to the above error, monitors will no longer be (de)activated.")
    }

    fn handle_expired(&mut self, res: Result<u64, AsyncError>) {
        if let Err(e) = res {
            log::error!("Could not wait for idle timer to expire: {}", ErrorFmt(e));
            self.dead = true;
            return;
        }
        let timeout = self.state.idle.timeout.get();
        let since = duration_since(self.last_input);
        if since >= timeout {
            if !timeout.is_zero() && !self.is_inhibited {
                self.backend.set_idle(true);
                self.idle = true;
            }
        } else {
            self.program_timer2(timeout - since);
        }
    }

    fn handle_idle_changes(&mut self) {
        if self.state.idle.inhibitors_changed.replace(false) {
            let is_inhibited = self.state.idle.inhibitors.len() > 0;
            if self.is_inhibited != is_inhibited {
                self.is_inhibited = is_inhibited;
                if !self.is_inhibited {
                    self.program_timer();
                }
            }
        }
        if self.state.idle.timeout_changed.replace(false) {
            self.program_timer();
        }
        if self.state.idle.input.replace(false) {
            self.last_input = now();
            if self.idle {
                self.backend.set_idle(false);
                self.idle = false;
                self.program_timer();
            }
        }
    }

    fn program_timer(&mut self) {
        self.program_timer2(self.state.idle.timeout.get());
    }

    fn program_timer2(&mut self, timeout: Duration) {
        if let Err(e) = self.timer.program(Some(timeout), None) {
            log::error!("Could not program idle timer: {}", ErrorFmt(e));
            self.dead = true;
        }
    }
}

fn now() -> c::timespec {
    let mut now = uapi::pod_zeroed();
    let _ = uapi::clock_gettime(c::CLOCK_MONOTONIC, &mut now);
    now
}

fn duration_since(start: c::timespec) -> Duration {
    let now = now();
    let mut nanos = (now.tv_sec as i64 - start.tv_sec as i64) * 1_000_000_000
        + (now.tv_nsec as i64 - start.tv_nsec as i64);
    if nanos < 0 {
        log::error!("Time has gone backwards.");
        nanos = 0;
    }
    Duration::from_nanos(nanos as u64)
}

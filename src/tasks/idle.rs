use {
    crate::{
        backend::transaction::{BackendConnectorTransactionError, ConnectorTransaction},
        state::State,
        utils::{
            errorfmt::ErrorFmt,
            timer::{TimerError, TimerFd},
        },
    },
    futures_util::{FutureExt, select},
    std::{rc::Rc, time::Duration},
    uapi::c,
};

pub async fn idle(state: Rc<State>) {
    let timer = match TimerFd::new(c::CLOCK_MONOTONIC) {
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
    timer: TimerFd,
    idle: bool,
    dead: bool,
    is_inhibited: bool,
    last_input: c::timespec,
}

impl Idle {
    async fn run(&mut self) {
        while !self.dead {
            select! {
                res = self.timer.expired(&self.state.ring).fuse() => self.handle_expired(res),
                _ = self.state.idle.change.triggered().fuse() => self.handle_idle_changes(),
            }
        }
        log::error!("Due to the above error, monitors will no longer be (de)activated.")
    }

    fn handle_expired(&mut self, res: Result<u64, TimerError>) {
        if let Err(e) = res {
            log::error!("Could not wait for idle timer to expire: {}", ErrorFmt(e));
            self.dead = true;
            return;
        }
        let grace_period = self.state.idle.grace_period.get();
        let timeout = self.state.idle.timeout.get();
        let after_grace = timeout.saturating_add(grace_period);
        let since = duration_since(self.last_input);
        if since >= after_grace {
            self.set_in_grace_period(false);
            if !timeout.is_zero() && !self.is_inhibited {
                if let Some(config) = self.state.config.get() {
                    config.idle();
                }
                self.set_idle(true);
                self.idle = true;
            }
        } else if since >= timeout {
            if !timeout.is_zero() && !self.is_inhibited {
                self.set_in_grace_period(true);
            }
            self.program_timer2(after_grace - since);
        } else {
            self.program_timer2(timeout - since);
        }
    }

    fn set_in_grace_period(&mut self, val: bool) {
        if self.state.idle.in_grace_period.replace(val) == val {
            return;
        }
        self.state.damage(self.state.root.extents.get());
        self.state.damage_hardware_cursors(false);
    }

    fn handle_idle_changes(&mut self) {
        if self.state.idle.inhibitors_changed.replace(false) {
            let is_inhibited = self.state.idle.inhibitors.len() > 0;
            if self.is_inhibited != is_inhibited {
                self.is_inhibited = is_inhibited;
                if !self.is_inhibited {
                    self.last_input = now();
                    self.program_timer();
                }
            }
        }
        if self.state.idle.timeout_changed.replace(false) {
            self.program_timer();
        }
        if self.state.idle.input.replace(false) {
            self.last_input = now();
            self.set_in_grace_period(false);
            if self.idle {
                self.set_idle(false);
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

    fn set_idle(&self, idle: bool) {
        if let Err(e) = self.try_set_idle(idle) {
            log::error!("Could not change idle status of backend: {}", ErrorFmt(e))
        }
        if let Some(lock) = self.state.lock.lock.get() {
            lock.check_locked();
        }
    }

    fn try_set_idle(&self, idle: bool) -> Result<(), BackendConnectorTransactionError> {
        let mut tran = ConnectorTransaction::new(&self.state);
        for connector in self.state.connectors.lock().values() {
            let mut state = connector.state.borrow().clone();
            state.active = !idle;
            tran.add(&connector.connector, state)?;
        }
        tran.prepare()?.apply()?.commit();
        self.state.set_backend_idle(idle);
        Ok(())
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

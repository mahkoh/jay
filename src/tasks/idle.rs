use crate::backend::transaction::BackendConnectorTransactionError;
use crate::backend::transaction::ConnectorTransaction;
use crate::io_uring::PendingTimeout;
use crate::io_uring::TimeoutCallback;
use crate::state::State;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::errorfmt::ErrorFmt;
use jay_algorithms::oserror::OsError;
use run_on_drop::on_drop;
use std::cell::Cell;
use std::rc::Rc;

pub async fn idle(state: Rc<State>) {
    state.idle.change.trigger();
    state.idle.timeout_changed.set(true);
    let idle = Rc::new(Idle {
        last_input: Cell::new(state.now_nsec()),
        state,
        timeout: Default::default(),
        idle: Default::default(),
        dead: Default::default(),
        is_inhibited: Default::default(),
    });
    let _reset_timeout = on_drop(|| {
        idle.timeout.take();
    });
    idle.run().await;
}

struct Idle {
    state: Rc<State>,
    timeout: Cell<Option<PendingTimeout>>,
    idle: Cell<bool>,
    dead: Cell<bool>,
    is_inhibited: Cell<bool>,
    last_input: Cell<u64>,
}

impl Idle {
    async fn run(self: &Rc<Self>) {
        while !self.dead.get() {
            self.state.idle.change.triggered().await;
            self.handle_idle_changes();
        }
        log::error!("Due to the above error, monitors will no longer be (de)activated.");
    }

    fn kill(&self) {
        self.dead.set(true);
        self.state.idle.change.trigger();
    }

    fn handle_expired(self: &Rc<Self>, res: Result<(), OsError>) {
        if let Err(e) = res {
            log::error!("Could not wait for idle timer to expire: {}", ErrorFmt(e));
            self.kill();
            return;
        }
        let now = self.state.now_nsec();
        let grace_period = self.state.idle.grace_period.get();
        let timeout_delta = self.state.idle.timeout.get();
        let timeout = self.last_input.get().saturating_add(timeout_delta);
        let after_grace = timeout.saturating_add(grace_period);
        if now >= after_grace {
            self.set_in_grace_period(false);
            if timeout_delta != 0 && !self.is_inhibited.get() {
                if let Some(config) = self.state.config.get() {
                    config.idle();
                }
                self.set_idle(true);
                self.idle.set(true);
            }
        } else if now >= timeout {
            if timeout_delta != 0 && !self.is_inhibited.get() {
                self.set_in_grace_period(true);
            }
            self.program_timer2(after_grace);
        } else {
            self.program_timer2(timeout);
        }
    }

    fn set_in_grace_period(&self, val: bool) {
        if self.state.idle.in_grace_period.replace(val) == val {
            return;
        }
        self.state.damage_full(RenderTL);
        self.state.damage_hardware_cursors(false);
    }

    fn handle_idle_changes(self: &Rc<Self>) {
        if self.state.idle.inhibitors_changed.replace(false) {
            let is_inhibited = self.state.idle.inhibitors.len() > 0;
            if self.is_inhibited.replace(is_inhibited) != is_inhibited && !is_inhibited {
                self.last_input.set(self.state.now_nsec());
                self.program_timer();
            }
        }
        if self.state.idle.timeout_changed.replace(false) {
            self.program_timer();
        }
        if self.state.idle.input.replace(false) {
            self.last_input.set(self.state.now_nsec());
            if self.state.idle.in_grace_period.get() {
                self.program_timer();
            }
            self.set_in_grace_period(false);
            if self.idle.get() {
                self.set_idle(false);
                self.idle.set(false);
                self.program_timer();
            }
        }
    }

    fn program_timer(self: &Rc<Self>) {
        let timeout = self
            .last_input
            .get()
            .saturating_add(self.state.idle.timeout.get());
        self.program_timer2(timeout);
    }

    fn program_timer2(self: &Rc<Self>, timeout: u64) {
        let res = self.state.ring.timeout_external(timeout, self.clone(), 0);
        match res {
            Ok(p) => self.timeout.set(Some(p)),
            Err(e) => {
                log::error!("Could not program idle timer: {}", ErrorFmt(e));
                self.kill();
            }
        }
    }

    fn set_idle(&self, idle: bool) {
        if let Err(e) = self.try_set_idle(idle) {
            log::error!("Could not change idle status of backend: {}", ErrorFmt(e));
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
        if !idle {
            let seats: Vec<_> = self.state.globals.seats.lock().values().cloned().collect();
            for seat in seats {
                if seat.get_keyboard_node().node_is_display() {
                    seat.focus_prev();
                }
            }
        }
        Ok(())
    }
}

impl TimeoutCallback for Idle {
    fn completed(self: Rc<Self>, res: Result<(), OsError>, _data: u64) {
        self.handle_expired(res);
    }
}

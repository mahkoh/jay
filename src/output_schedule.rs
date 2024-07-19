use {
    crate::{
        async_engine::AsyncEngine,
        backend::{Connector, HardwareCursor},
        ifs::wl_output::PersistentOutputState,
        io_uring::{IoUring, IoUringError},
        utils::{
            asyncevent::AsyncEvent, cell_ext::CellExt, clonecell::CloneCell, errorfmt::ErrorFmt,
            numcell::NumCell,
        },
    },
    futures_util::{select, FutureExt},
    num_traits::ToPrimitive,
    std::{cell::Cell, rc::Rc},
};

pub struct OutputSchedule {
    changed: AsyncEvent,
    run: Cell<bool>,

    connector: Rc<dyn Connector>,
    hardware_cursor: CloneCell<Option<Rc<dyn HardwareCursor>>>,

    persistent: Rc<PersistentOutputState>,

    last_present_nsec: Cell<u64>,
    cursor_delta_nsec: Cell<Option<u64>>,

    ring: Rc<IoUring>,
    eng: Rc<AsyncEngine>,

    vrr_enabled: Cell<bool>,

    present_scheduled: Cell<bool>,
    needs_hardware_cursor_commit: Cell<bool>,
    needs_software_cursor_damage: Cell<bool>,

    iteration: NumCell<u64>,
}

impl OutputSchedule {
    pub fn new(
        ring: &Rc<IoUring>,
        eng: &Rc<AsyncEngine>,
        connector: &Rc<dyn Connector>,
        persistent: &Rc<PersistentOutputState>,
    ) -> Self {
        let slf = Self {
            changed: Default::default(),
            run: Default::default(),
            connector: connector.clone(),
            ring: ring.clone(),
            eng: eng.clone(),
            vrr_enabled: Default::default(),
            present_scheduled: Cell::new(true),
            needs_hardware_cursor_commit: Default::default(),
            needs_software_cursor_damage: Default::default(),
            hardware_cursor: Default::default(),
            persistent: persistent.clone(),
            last_present_nsec: Default::default(),
            cursor_delta_nsec: Default::default(),
            iteration: Default::default(),
        };
        if let Some(hz) = persistent.vrr_cursor_hz.get() {
            slf.set_cursor_hz(hz);
        }
        slf
    }

    pub async fn drive(self: Rc<Self>) {
        loop {
            self.run_once().await;
            while !self.run.take() {
                self.changed.triggered().await;
            }
        }
    }

    fn trigger(&self) {
        let trigger = self.vrr_enabled.get()
            && !self.present_scheduled.get()
            && self.cursor_delta_nsec.is_some()
            && (self.needs_software_cursor_damage.get() || self.needs_hardware_cursor_commit.get());
        if trigger {
            self.run.set(true);
            self.changed.trigger();
        }
    }

    pub fn presented(&self) {
        self.last_present_nsec.set(self.eng.now().nsec());
        self.present_scheduled.set(false);
        self.iteration.fetch_add(1);
        self.trigger();
    }

    pub fn vrr_enabled(&self) -> bool {
        self.vrr_enabled.get()
    }

    pub fn set_vrr_enabled(&self, enabled: bool) {
        self.vrr_enabled.set(enabled);
        self.trigger();
    }

    pub fn set_cursor_hz(&self, hz: f64) {
        let (hz, delta) = match map_cursor_hz(hz) {
            None => {
                log::warn!("Ignoring cursor frequency {hz}");
                return;
            }
            Some(v) => v,
        };
        self.persistent.vrr_cursor_hz.set(hz);
        self.cursor_delta_nsec.set(delta);
        self.trigger();
    }

    pub fn set_hardware_cursor(&self, hc: &Option<Rc<dyn HardwareCursor>>) {
        self.hardware_cursor.set(hc.clone());
    }

    pub fn defer_cursor_updates(&self) -> bool {
        self.vrr_enabled.get() && self.cursor_delta_nsec.is_some()
    }

    pub fn hardware_cursor_changed(&self) {
        if !self.needs_hardware_cursor_commit.replace(true) {
            self.trigger();
        }
    }

    pub fn software_cursor_changed(&self) {
        if !self.needs_software_cursor_damage.replace(true) {
            self.trigger();
        }
    }

    async fn run_once(&self) {
        if self.present_scheduled.get() {
            return;
        }
        if !self.needs_hardware_cursor_commit.get() && !self.needs_software_cursor_damage.get() {
            return;
        }
        loop {
            if !self.vrr_enabled.get() {
                return;
            }
            let Some(duration) = self.cursor_delta_nsec.get() else {
                return;
            };
            let iteration = self.iteration.get();
            let next_present = self.last_present_nsec.get().saturating_add(duration);
            let res: Result<(), IoUringError> = select! {
                _ = self.changed.triggered().fuse() => continue,
                v = self.ring.timeout(next_present).fuse() => v,
            };
            if let Err(e) = res {
                log::error!("Could not wait for timer to expire: {}", ErrorFmt(e));
                return;
            }
            if iteration == self.iteration.get() {
                break;
            }
        }
        if self.needs_hardware_cursor_commit.take() {
            if let Some(hc) = self.hardware_cursor.get() {
                if hc.schedule_present() {
                    self.present_scheduled.set(true);
                }
            }
        }
        if self.needs_software_cursor_damage.take() {
            self.connector.damage();
            self.present_scheduled.set(true);
        }
    }
}

pub fn map_cursor_hz(hz: f64) -> Option<(Option<f64>, Option<u64>)> {
    if hz <= 0.0 {
        return Some((Some(0.0), Some(u64::MAX)));
    }
    let delta = (1_000_000_000.0 / hz).to_u64();
    if delta.is_none() {
        if hz > 0.0 {
            return Some((None, None));
        }
        return None;
    }
    if delta == Some(0) {
        return Some((None, None));
    }
    Some((Some(hz), delta))
}

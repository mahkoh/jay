use {
    crate::{
        async_engine::AsyncEngine,
        backend::HardwareCursor,
        ifs::wl_output::PersistentOutputState,
        io_uring::{IoUring, IoUringError},
        state::ConnectorData,
        utils::{
            asyncevent::AsyncEvent, cell_ext::CellExt, clonecell::CloneCell, errorfmt::ErrorFmt,
            numcell::NumCell,
        },
    },
    futures_util::{FutureExt, select},
    num_traits::ToPrimitive,
    std::{cell::Cell, rc::Rc},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Change {
    /// The backend has applied the latest changes.
    None,
    /// There are changes that the backend is not yet aware of.
    Scheduled,
    /// The backend is aware that there are changes and will apply them as part of the
    /// next latch event.
    AwaitingLatch,
}

pub struct OutputSchedule {
    changed: AsyncEvent,
    run: Cell<bool>,

    connector: Rc<ConnectorData>,
    hardware_cursor: CloneCell<Option<Rc<dyn HardwareCursor>>>,

    persistent: Rc<PersistentOutputState>,

    last_present_nsec: Cell<u64>,
    cursor_delta_nsec: Cell<Option<u64>>,

    ring: Rc<IoUring>,
    eng: Rc<AsyncEngine>,

    vrr_enabled: Cell<bool>,

    hardware_cursor_change: Cell<Change>,
    software_cursor_change: Cell<Change>,

    iteration: NumCell<u64>,
}

impl OutputSchedule {
    pub fn new(
        ring: &Rc<IoUring>,
        eng: &Rc<AsyncEngine>,
        connector: &Rc<ConnectorData>,
        persistent: &Rc<PersistentOutputState>,
    ) -> Self {
        let slf = Self {
            changed: Default::default(),
            run: Default::default(),
            connector: connector.clone(),
            ring: ring.clone(),
            eng: eng.clone(),
            vrr_enabled: Default::default(),
            hardware_cursor_change: Cell::new(Change::None),
            software_cursor_change: Cell::new(Change::None),
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
            && self.cursor_delta_nsec.is_some()
            && (self.software_cursor_change.get() == Change::Scheduled
                || self.hardware_cursor_change.get() == Change::Scheduled);
        if trigger {
            self.run.set(true);
            self.changed.trigger();
        }
    }

    pub fn latched(&self) {
        self.last_present_nsec.set(self.eng.now().nsec());
        if self.software_cursor_change.get() == Change::AwaitingLatch {
            self.software_cursor_change.set(Change::None);
        }
        if self.hardware_cursor_change.get() == Change::AwaitingLatch {
            self.hardware_cursor_change.set(Change::None);
        }
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
        if self.hardware_cursor_change.get() == Change::None {
            self.hardware_cursor_change.set(Change::Scheduled);
            self.trigger();
        }
    }

    pub fn software_cursor_changed(&self) {
        if self.software_cursor_change.get() == Change::None {
            self.software_cursor_change.set(Change::Scheduled);
            self.trigger();
        }
    }

    async fn run_once(&self) {
        loop {
            if self.hardware_cursor_change.get() != Change::Scheduled
                && self.software_cursor_change.get() != Change::Scheduled
            {
                return;
            }
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
        self.commit_cursor();
    }

    pub fn commit_cursor(&self) {
        if self.hardware_cursor_change.get() == Change::Scheduled {
            if let Some(hc) = self.hardware_cursor.get() {
                hc.damage();
            }
            self.hardware_cursor_change.set(Change::AwaitingLatch);
        }
        if self.software_cursor_change.get() == Change::Scheduled {
            self.connector.damage();
            self.software_cursor_change.set(Change::AwaitingLatch);
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

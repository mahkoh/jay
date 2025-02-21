use crate::utils::numcell::NumCell;

#[derive(Default)]
pub struct ThresholdCounter {
    counter: NumCell<usize>,
}

impl ThresholdCounter {
    pub fn inc(&self) -> bool {
        self.counter.fetch_add(1) == 0
    }

    pub fn dec(&self) -> bool {
        self.counter.fetch_sub(1) == 1
    }

    pub fn adj(&self, inc: bool) -> bool {
        if inc { self.inc() } else { self.dec() }
    }

    pub fn active(&self) -> bool {
        self.counter.get() > 0
    }
}

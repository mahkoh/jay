use {
    crate::{
        backend::AXIS_120,
        ifs::wl_seat::{
            wl_pointer::{PendingScroll, VERTICAL_SCROLL},
            PX_PER_SCROLL,
        },
    },
    std::cell::Cell,
};

#[derive(Default)]
pub struct Scroller {
    v120: Cell<i32>,
    smooth: Cell<f64>,
}

impl Scroller {
    pub fn handle(&self, scroll: &PendingScroll) -> Option<i32> {
        let n = if let Some(d) = scroll.v120[VERTICAL_SCROLL as usize].get() {
            self.smooth.set(0.0);
            let mut v120 = self.v120.get() + d;
            let discrete = v120 / AXIS_120;
            v120 -= discrete * AXIS_120;
            self.v120.set(v120);
            discrete
        } else if let Some(smooth) = scroll.smooth[VERTICAL_SCROLL as usize].get() {
            self.v120.set(0);
            let mut smooth = self.smooth.get() + smooth.to_f64();
            let discrete = (smooth / PX_PER_SCROLL).trunc();
            smooth -= discrete * PX_PER_SCROLL;
            self.smooth.set(smooth);
            discrete as _
        } else {
            0
        };
        if scroll.stop[VERTICAL_SCROLL as usize].get() {
            self.v120.set(0);
            self.smooth.set(0.0);
        }
        if n != 0 {
            Some(n)
        } else {
            None
        }
    }
}

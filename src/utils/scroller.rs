use {
    crate::ifs::wl_seat::{
        wl_pointer::{PendingScroll, VERTICAL_SCROLL},
        PX_PER_SCROLL,
    },
    std::cell::Cell,
};

#[derive(Default)]
pub struct Scroller {
    scroll: Cell<f64>,
}

impl Scroller {
    pub fn handle(&self, scroll: &PendingScroll) -> Option<i32> {
        if let Some(d) = scroll.discrete[VERTICAL_SCROLL as usize].get() {
            self.scroll.set(0.0);
            Some(d)
        } else if let Some(scroll) = scroll.axis[VERTICAL_SCROLL as usize].get() {
            let mut scroll = self.scroll.get() + scroll.to_f64();
            let discrete = (scroll / PX_PER_SCROLL).trunc();
            scroll -= discrete * PX_PER_SCROLL;
            self.scroll.set(scroll);
            Some(discrete as i32)
        } else {
            None
        }
    }
}

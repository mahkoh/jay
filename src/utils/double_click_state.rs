use crate::state::State;

#[derive(Default)]
pub struct DoubleClickState {
    last_click: Option<(u64, i32, i32)>,
}

impl DoubleClickState {
    pub fn click(&mut self, state: &State, time_usec: u64, x: i32, y: i32) -> bool {
        let res = self.click_(state, time_usec, x, y);
        if !res {
            self.last_click = Some((time_usec, x, y));
        }
        res
    }

    fn click_(&mut self, state: &State, time_usec: u64, x: i32, y: i32) -> bool {
        let Some((last_usec, last_x, last_y)) = self.last_click.take() else {
            return false;
        };
        if time_usec.wrapping_sub(last_usec) > state.double_click_interval_usec.get() {
            return false;
        }
        let max_dist = state.double_click_distance.get();
        if max_dist < 0 {
            return false;
        }
        let dist_x = last_x - x;
        let dist_y = last_y - y;
        if dist_x * dist_x + dist_y * dist_y > max_dist * max_dist {
            return false;
        }
        true
    }
}

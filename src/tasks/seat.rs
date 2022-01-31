use crate::backend::Seat;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::utils::asyncevent::AsyncEvent;
use crate::State;
use std::rc::Rc;

pub struct SeatHandler {
    pub state: Rc<State>,
    pub seat: Rc<dyn Seat>,
    pub tree_changed: Rc<AsyncEvent>,
}

impl SeatHandler {
    pub async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.seat.on_change(Rc::new(move || ae.trigger()));
        }
        let name = self.state.globals.name();
        let global = Rc::new(WlSeatGlobal::new(name, &self.state, &self.seat));
        let _tree_changed = self
            .state
            .eng
            .spawn(tree_changed(self.state.clone(), global.clone(), self.tree_changed.clone()));
        let mut _node = self.state.seat_queue.add_last(global.clone());
        self.state.add_global(&global);
        loop {
            if self.seat.removed() {
                break;
            }
            let mut any_events = false;
            while let Some(event) = self.seat.event() {
                global.event(event);
                any_events = true;
            }
            if any_events {
                _node = self.state.seat_queue.add_last(global.clone());
            }
            ae.triggered().await;
        }
        global.set_cursor(None);
        let _ = self.state.globals.remove(&self.state, name);
        self.state.seats.borrow_mut().remove(&self.seat.id());
    }
}

async fn tree_changed(state: Rc<State>, global: Rc<WlSeatGlobal>, tree_changed: Rc<AsyncEvent>) {
    loop {
        tree_changed.triggered().await;
        state.tree_changed_sent.set(false);
        global.tree_changed();
    }
}

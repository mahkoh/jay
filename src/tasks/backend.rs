use crate::backend::{BackendEvent, Output, Seat};
use crate::state::SeatData;
use crate::tasks::output::OutputHandler;
use crate::tasks::seat::SeatHandler;
use crate::utils::asyncevent::AsyncEvent;
use crate::State;
use std::rc::Rc;

pub struct BackendEventHandler {
    pub state: Rc<State>,
}

impl BackendEventHandler {
    pub async fn handle_events(&mut self) {
        loop {
            let event = self.state.backend_events.pop().await;
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::NewOutput(output) => self.handle_new_output(output),
            BackendEvent::NewSeat(seat) => self.handle_new_seat(seat),
        }
    }

    fn handle_new_output(&mut self, output: Rc<dyn Output>) {
        let id = output.id();
        let oh = OutputHandler {
            state: self.state.clone(),
            output,
        };
        let future = self.state.eng.spawn(oh.handle());
        self.state.output_handlers.borrow_mut().insert(id, future);
    }

    fn handle_new_seat(&mut self, seat: Rc<dyn Seat>) {
        let id = seat.id();
        let tree_changed = Rc::new(AsyncEvent::default());
        let oh = SeatHandler {
            state: self.state.clone(),
            seat,
            tree_changed: tree_changed.clone(),
        };
        let handler = self.state.eng.spawn(oh.handle());
        self.state.seats.borrow_mut().insert(
            id,
            SeatData {
                handler,
                tree_changed,
            },
        );
    }
}

use crate::backend::{BackendEvent, Output};
use crate::state::State;
use crate::tasks::input_device;
use crate::tasks::output::OutputHandler;
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
            BackendEvent::NewInputDevice(s) => input_device::handle(&self.state, s),
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
}
use {
    crate::{
        backend::BackendEvent,
        state::State,
        tasks::{connector, input_device},
    },
    std::rc::Rc,
};

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
            BackendEvent::NewConnector(connector) => connector::handle(&self.state, &connector),
            BackendEvent::NewInputDevice(s) => input_device::handle(&self.state, s),
        }
    }
}

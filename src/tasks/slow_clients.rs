use {crate::state::State, std::rc::Rc};

pub struct SlowClientHandler {
    pub state: Rc<State>,
}

impl SlowClientHandler {
    pub async fn handle_events(&mut self) {
        loop {
            let client = self.state.slow_clients.pop().await;
            client.check_queue_size().await;
        }
    }
}

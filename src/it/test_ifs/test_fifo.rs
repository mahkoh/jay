use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::wire::WpFifoV1Id;
use std::rc::Rc;

pub struct TestFifo {
    pub id: WpFifoV1Id,
    pub client: Rc<Client>,
}

impl TestFifo {
    pub fn set_barrier(&self) {
        self.client.send_wp_fifo_v1_set_barrier(self.id);
    }

    pub fn wait_barrier(&self) {
        self.client.send_wp_fifo_v1_wait_barrier(self.id);
    }
}

impl TestClient {
    pub fn get_fifo(&self, surface: &TestSurface) -> Rc<TestFifo> {
        let id = self.client.send_wp_fifo_manager_v1_get_fifo(surface.id);
        Rc::new(TestFifo {
            id,
            client: self.client.clone(),
        })
    }
}

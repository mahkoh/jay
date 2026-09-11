use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::wire::ZwlrDataControlSourceV1Id;
use crate::wire::zwlr_data_control_source_v1::*;
use std::cell::Cell;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct TestDataControlSource {
    pub id: ZwlrDataControlSourceV1Id,
    pub client: Rc<Client>,
    pub cancelled: Cell<bool>,
    pub sends: TEEH<(String, Rc<OwnedFd>)>,
}

impl TestDataControlSource {
    pub fn offer(&self, mime_type: &str) {
        self.client
            .send_zwlr_data_control_source_v1_offer(self.id, mime_type);
    }
}

synthetic_event_handler!(TestDataControlSource);

impl ZwlrDataControlSourceV1EventHandler for TestDataControlSource {
    type Error = TestErrorError;

    fn send(&self, ev: Send<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.sends.push((ev.mime_type.to_string(), ev.fd));
        Ok(())
    }

    fn cancelled(&self, _ev: Cancelled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.cancelled.set(true);
        Ok(())
    }
}

impl TestClient {
    pub fn create_data_control_source(&self) -> Rc<TestDataControlSource> {
        let client = &self.client;
        let id = client.send_zwlr_data_control_manager_v1_create_data_source();
        let source = Rc::new(TestDataControlSource {
            id,
            client: client.clone(),
            cancelled: Cell::new(false),
            sends: Default::default(),
        });
        client.set_synthetic_event_handler(id, &source);
        source
    }
}

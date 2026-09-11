use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::wire::WlDataSourceId;
use crate::wire::wl_data_source::*;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct TestDataSource {
    pub id: WlDataSourceId,
    pub client: Rc<Client>,
    pub sends: TEEH<(String, Rc<OwnedFd>)>,
}

impl TestDataSource {
    pub fn offer(&self, mime_type: &str) {
        self.client.send_wl_data_source_offer(self.id, mime_type);
    }

    pub fn set_actions(&self, actions: u32) {
        self.client
            .send_wl_data_source_set_actions(self.id, actions);
    }
}

synthetic_event_handler!(TestDataSource);

impl WlDataSourceEventHandler for TestDataSource {
    type Error = TestErrorError;

    fn target(&self, _ev: Target<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn send(&self, ev: Send<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.sends.push((ev.mime_type.to_string(), ev.fd));
        Ok(())
    }

    fn cancelled(&self, _ev: Cancelled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn dnd_drop_performed(
        &self,
        _ev: DndDropPerformed,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn dnd_finished(&self, _ev: DndFinished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn action(&self, _ev: Action, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl TestClient {
    pub fn create_data_source(&self) -> Rc<TestDataSource> {
        let client = &self.client;
        let id = client.send_wl_data_device_manager_create_data_source();
        let source = Rc::new(TestDataSource {
            id,
            client: client.clone(),
            sends: Rc::new(Default::default()),
        });
        client.set_synthetic_event_handler(id, &source);
        source
    }
}

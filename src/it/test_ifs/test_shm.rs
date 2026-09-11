use crate::client::Client;
use crate::globals::Singleton;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestErrorError;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::WlShmId;
use crate::wire::wl_shm::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestShm {
    client: Rc<Client>,
    formats: CopyHashMap<u32, ()>,
    formats_awaited: Cell<bool>,
}

impl TestClient {
    pub fn new_shm(&self) -> Rc<TestShm> {
        let id: WlShmId = self.client.bind(Singleton::WlShm);
        let slf = Rc::new(TestShm {
            client: self.client.clone(),
            formats: Default::default(),
            formats_awaited: Cell::new(false),
        });
        self.client.set_synthetic_event_handler(id, &slf);
        slf
    }
}

impl TestShm {
    pub async fn formats(&self) -> &CopyHashMap<u32, ()> {
        if !self.formats_awaited.replace(true) {
            self.client.sync().await;
        }
        &self.formats
    }
}

synthetic_event_handler!(TestShm);

impl WlShmEventHandler for TestShm {
    type Error = TestErrorError;

    fn format(&self, ev: Format, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.formats.set(ev.format, ());
        Ok(())
    }
}

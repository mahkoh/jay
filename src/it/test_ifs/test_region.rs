use crate::client::Client;
use crate::ifs::wl_region::WlRegion;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::rect::Rect;
use crate::rect::RegionBuilder;
use crate::wire::WlRegionId;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestRegion {
    pub id: WlRegionId,
    pub client: Rc<Client>,
    pub server: Rc<WlRegion>,
    pub expected: RefCell<RegionBuilder>,
}

impl TestRegion {
    pub fn add(&self, rect: Rect) {
        self.expected.borrow_mut().add(rect);
        self.client
            .send_wl_region_add(self.id, rect.x1(), rect.y1(), rect.width(), rect.height());
    }

    pub fn subtract(&self, rect: Rect) {
        self.expected.borrow_mut().sub(rect);
        self.client.send_wl_region_subtract(
            self.id,
            rect.x1(),
            rect.y1(),
            rect.width(),
            rect.height(),
        );
    }

    pub async fn check(&self) -> Result<(), TestError> {
        self.client.sync().await;
        let expected = self.expected.borrow_mut().get();
        let actual = self.server.region();
        tassert_eq!(expected, actual);
        Ok(())
    }
}

impl TestClient {
    pub async fn create_region(&self) -> TestResult<Rc<TestRegion>> {
        let client = &self.client;
        let id = client.send_wl_compositor_create_region();
        client.sync().await;
        Ok(Rc::new(TestRegion {
            id,
            client: client.clone(),
            server: client.lookup(id)?,
            expected: Default::default(),
        }))
    }
}

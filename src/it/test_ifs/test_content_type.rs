use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::wire::WpContentTypeV1Id;
use std::rc::Rc;

pub struct TestContentType {
    pub id: WpContentTypeV1Id,
    pub client: Rc<Client>,
}

impl TestContentType {
    pub fn set_content_type(&self, content_type: u32) {
        self.client
            .send_wp_content_type_v1_set_content_type(self.id, content_type);
    }
}

impl TestClient {
    pub fn get_surface_content_type(&self, surface: &TestSurface) -> Rc<TestContentType> {
        let id = self
            .client
            .send_wp_content_type_manager_v1_get_surface_content_type(surface.id);
        Rc::new(TestContentType {
            id,
            client: self.client.clone(),
        })
    }
}

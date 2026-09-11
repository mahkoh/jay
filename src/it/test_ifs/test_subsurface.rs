use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::wire::WlSubsurfaceId;
use crate::wire::WlSurfaceId;
use std::rc::Rc;

pub struct TestSubsurface {
    pub id: WlSubsurfaceId,
    pub client: Rc<Client>,
}

impl TestSubsurface {
    pub fn set_position(&self, x: i32, y: i32) {
        self.client.send_wl_subsurface_set_position(self.id, x, y);
    }

    pub fn place_above(&self, surface: WlSurfaceId) {
        self.client.send_wl_subsurface_place_above(self.id, surface);
    }

    pub fn place_below(&self, surface: WlSurfaceId) {
        self.client.send_wl_subsurface_place_below(self.id, surface);
    }

    #[expect(unused)]
    pub fn set_sync(&self) {
        self.client.send_wl_subsurface_set_sync(self.id);
    }

    pub fn set_desync(&self) {
        self.client.send_wl_subsurface_set_desync(self.id);
    }
}

impl TestClient {
    pub fn get_subsurface(&self, surface: WlSurfaceId, parent: WlSurfaceId) -> Rc<TestSubsurface> {
        let id = self
            .client
            .send_wl_subcompositor_get_subsurface(surface, parent);
        Rc::new(TestSubsurface {
            id,
            client: self.client.clone(),
        })
    }
}

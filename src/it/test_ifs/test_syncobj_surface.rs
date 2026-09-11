use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_ifs::test_syncobj_timeline::TestSyncobjTimeline;
use crate::wire::WpLinuxDrmSyncobjSurfaceV1Id;
use std::rc::Rc;

pub struct TestSyncobjSurface {
    pub id: WpLinuxDrmSyncobjSurfaceV1Id,
    pub client: Rc<Client>,
}

impl TestSyncobjSurface {
    pub fn destroy(&self) {
        self.client
            .send_wp_linux_drm_syncobj_surface_v1_destroy(self.id);
    }

    pub fn set_acquire_point(&self, tl: &TestSyncobjTimeline, point: u64) {
        self.client
            .send_wp_linux_drm_syncobj_surface_v1_set_acquire_point(self.id, tl.id, point);
    }

    pub fn set_release_point(&self, tl: &TestSyncobjTimeline, point: u64) {
        self.client
            .send_wp_linux_drm_syncobj_surface_v1_set_release_point(self.id, tl.id, point);
    }
}

impl TestClient {
    pub fn get_syncobj_surface(&self, surface: &TestSurface) -> Rc<TestSyncobjSurface> {
        let id = self
            .client
            .send_wp_linux_drm_syncobj_manager_v1_get_surface(surface.id);
        Rc::new(TestSyncobjSurface {
            id,
            client: self.client.clone(),
        })
    }
}

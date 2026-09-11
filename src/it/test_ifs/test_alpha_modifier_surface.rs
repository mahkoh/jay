use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::wire::WpAlphaModifierSurfaceV1Id;
use std::rc::Rc;

pub struct TestAlphaModifierSurface {
    pub id: WpAlphaModifierSurfaceV1Id,
    pub client: Rc<Client>,
}

impl TestAlphaModifierSurface {
    pub fn set_multiplier(&self, factor: f64) {
        self.client
            .send_wp_alpha_modifier_surface_v1_set_multiplier(
                self.id,
                (factor * u32::MAX as f64) as u32,
            );
    }
}

impl TestClient {
    pub fn get_alpha_modifier_surface(
        &self,
        surface: &TestSurface,
    ) -> Rc<TestAlphaModifierSurface> {
        let id = self
            .client
            .send_wp_alpha_modifier_v1_get_surface(surface.id);
        Rc::new(TestAlphaModifierSurface {
            id,
            client: self.client.clone(),
        })
    }
}

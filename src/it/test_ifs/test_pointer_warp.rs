use crate::fixed::Fixed;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_pointer::TestPointer;
use crate::it::test_ifs::test_surface::TestSurface;

impl TestClient {
    pub fn warp_pointer(
        &self,
        surface: &TestSurface,
        pointer: &TestPointer,
        x: Fixed,
        y: Fixed,
        serial: u32,
    ) {
        self.client
            .send_wp_pointer_warp_v1_warp_pointer(surface.id, pointer.id, x, y, serial);
    }
}

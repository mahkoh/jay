use crate::cmm::cmm_eotf::Eotf;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::theme::Color;
use crate::wire::WlBufferId;
use crate::wire::wl_buffer::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestBuffer {
    pub id: WlBufferId,
    pub released: Cell<bool>,
}

synthetic_event_handler!(TestBuffer);

impl WlBufferEventHandler for TestBuffer {
    type Error = TestErrorError;

    fn release(&self, _ev: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.released.set(true);
        Ok(())
    }
}

impl TestClient {
    pub fn create_single_pixel_buffer(&self, color: Color) -> Rc<TestBuffer> {
        let map = |c: f32| (c as f64 * u32::MAX as f64) as u32;
        let [r, g, b, a] = color.to_array(Eotf::Gamma22);
        let client = &self.client;
        let id = client.send_wp_single_pixel_buffer_manager_v1_create_u32_rgba_buffer(
            map(r),
            map(g),
            map(b),
            map(a),
        );
        let buffer = Rc::new(TestBuffer {
            id,
            released: Cell::new(true),
        });
        client.set_synthetic_event_handler(id, &buffer);
        buffer
    }
}

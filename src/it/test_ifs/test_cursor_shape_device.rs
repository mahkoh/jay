use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_pointer::TestPointer;
use crate::wire::WpCursorShapeDeviceV1Id;
use std::rc::Rc;

pub struct TestCursorShapeDevice {
    pub id: WpCursorShapeDeviceV1Id,
    pub client: Rc<Client>,
}

impl TestCursorShapeDevice {
    pub fn set_shape(&self, serial: u32, shape: u32) {
        self.client
            .send_wp_cursor_shape_device_v1_set_shape(self.id, serial, shape);
    }
}

impl TestClient {
    pub fn get_cursor_shape_device(&self, pointer: &TestPointer) -> Rc<TestCursorShapeDevice> {
        let id = self
            .client
            .send_wp_cursor_shape_manager_v1_get_pointer(pointer.id);
        Rc::new(TestCursorShapeDevice {
            id,
            client: self.client.clone(),
        })
    }
}

use crate::client::Client;
use crate::fixed::Fixed;
use crate::wire::WpViewportId;
use std::rc::Rc;

pub struct TestViewport {
    pub id: WpViewportId,
    pub client: Rc<Client>,
}

impl TestViewport {
    pub fn set_source(&self, x: i32, y: i32, width: i32, height: i32) {
        self.client.send_wp_viewport_set_source(
            self.id,
            Fixed::from_int(x),
            Fixed::from_int(y),
            Fixed::from_int(width),
            Fixed::from_int(height),
        );
    }

    pub fn set_destination(&self, width: i32, height: i32) {
        self.client
            .send_wp_viewport_set_destination(self.id, width.max(1), height.max(1));
    }
}

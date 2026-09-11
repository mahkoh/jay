use crate::client::Client;
use crate::ifs::wl_surface::WlSurface;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_callback::TestCallback;
use crate::it::test_ifs::test_region::TestRegion;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::wire::WlBufferId;
use crate::wire::WlSurfaceId;
use crate::wire::wl_surface::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestSurface {
    pub id: WlSurfaceId,
    pub client: Rc<Client>,
    pub server: Rc<WlSurface>,
    pub preferred_buffer_scale: TEEH<i32>,
    pub preferred_buffer_transform: TEEH<u32>,
}

impl TestSurface {
    pub fn attach(&self, buffer_id: WlBufferId) {
        self.client.send_wl_surface_attach(self.id, buffer_id, 0, 0);
    }

    pub fn offset(&self, dx: i32, dy: i32) {
        self.client.send_wl_surface_offset(self.id, dx, dy);
    }

    pub fn set_input_region(&self, region: &TestRegion) {
        self.client
            .send_wl_surface_set_input_region(self.id, region.id);
    }

    pub fn damage(&self, x: i32, y: i32, width: i32, height: i32) {
        self.client
            .send_wl_surface_damage(self.id, x, y, width, height);
    }

    pub fn damage_buffer(&self, x: i32, y: i32, width: i32, height: i32) {
        self.client
            .send_wl_surface_damage_buffer(self.id, x, y, width, height);
    }

    pub fn set_buffer_transform(&self, transform: i32) {
        self.client
            .send_wl_surface_set_buffer_transform(self.id, transform);
    }

    pub fn frame(&self) -> Rc<TestCallback> {
        let id = self.client.send_wl_surface_frame(self.id);
        let callback = Rc::new(TestCallback {
            handler: Cell::new(None),
            done: Cell::new(false),
        });
        self.client.set_synthetic_event_handler(id, &callback);
        callback
    }

    pub fn commit(&self) {
        self.client.send_wl_surface_commit(self.id);
    }
}

synthetic_event_handler!(TestSurface);

impl WlSurfaceEventHandler for TestSurface {
    type Error = TestErrorError;

    fn enter(&self, _ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn leave(&self, _ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn preferred_buffer_scale(
        &self,
        ev: PreferredBufferScale,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.preferred_buffer_scale.push(ev.factor);
        Ok(())
    }

    fn preferred_buffer_transform(
        &self,
        ev: PreferredBufferTransform,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.preferred_buffer_transform.push(ev.transform);
        Ok(())
    }
}

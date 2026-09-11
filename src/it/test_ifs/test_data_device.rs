use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_data_offer::TestDataOffer;
use crate::it::test_ifs::test_data_source::TestDataSource;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::wire::WlDataDeviceId;
use crate::wire::WlSurfaceId;
use crate::wire::wl_data_device::*;
use std::rc::Rc;

pub struct TestDataDevice {
    pub id: WlDataDeviceId,
    pub client: Rc<Client>,
}

impl TestDataDevice {
    pub fn start_drag(
        &self,
        source: &TestDataSource,
        origin: &TestSurface,
        icon: Option<&TestSurface>,
        serial: u32,
    ) {
        self.client.send_wl_data_device_start_drag(
            self.id,
            source.id,
            origin.id,
            icon.map(|i| i.id).unwrap_or(WlSurfaceId::NONE),
            serial,
        );
    }

    pub fn set_selection(&self, source: &TestDataSource, serial: u32) {
        self.client
            .send_wl_data_device_set_selection(self.id, source.id, serial);
    }
}

synthetic_event_handler!(TestDataDevice);

impl WlDataDeviceEventHandler for TestDataDevice {
    type Error = TestErrorError;

    fn data_offer(&self, ev: DataOffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let offer = Rc::new(TestDataOffer {
            id: ev.id,
            client: self.client.clone(),
        });
        self.client.set_synthetic_event_handler(ev.id, &offer);
        offer.destroy();
        Ok(())
    }

    fn enter(&self, _ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn leave(&self, _ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn motion(&self, _ev: Motion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn drop_(&self, _ev: Drop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn selection(&self, _ev: Selection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl TestClient {
    pub fn get_data_device(&self, seat: &TestSeat) -> Rc<TestDataDevice> {
        let client = &self.client;
        let id = client.send_wl_data_device_manager_get_data_device(seat.id);
        let dev = Rc::new(TestDataDevice {
            id,
            client: client.clone(),
        });
        client.set_synthetic_event_handler(id, &dev);
        dev
    }
}

use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_data_control_offer::TestDataControlOffer;
use crate::it::test_ifs::test_data_control_source::TestDataControlSource;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::ZwlrDataControlDeviceV1Id;
use crate::wire::ZwlrDataControlOfferV1Id;
use crate::wire::zwlr_data_control_device_v1::*;
use std::rc::Rc;

pub struct TestDataControlDevice {
    pub id: ZwlrDataControlDeviceV1Id,
    pub client: Rc<Client>,
    pub pending_offer: CopyHashMap<ZwlrDataControlOfferV1Id, Rc<TestDataControlOffer>>,
    pub selection: TEEH<Option<Rc<TestDataControlOffer>>>,
    pub primary_selection: TEEH<Option<Rc<TestDataControlOffer>>>,
}

impl TestDataControlDevice {
    pub fn set_selection(&self, source: &TestDataControlSource) {
        self.client
            .send_zwlr_data_control_device_v1_set_selection(self.id, source.id);
    }

    #[expect(unused)]
    pub fn set_primary_selection(&self, source: &TestDataControlSource) {
        self.client
            .send_zwlr_data_control_device_v1_set_primary_selection(self.id, source.id);
    }

    fn take_offer(
        &self,
        id: ZwlrDataControlOfferV1Id,
    ) -> TestResult<Option<Rc<TestDataControlOffer>>> {
        if id.is_none() {
            Ok(None)
        } else {
            match self.pending_offer.remove(&id) {
                Some(o) => Ok(Some(o)),
                _ => bail!("Unknown offer {}", id),
            }
        }
    }
}

synthetic_event_handler!(TestDataControlDevice);

impl ZwlrDataControlDeviceV1EventHandler for TestDataControlDevice {
    type Error = TestErrorError;

    fn data_offer(&self, ev: DataOffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(TestDataControlOffer {
            id: ev.id,
            client: self.client.clone(),
            offers: Default::default(),
        });
        self.client.set_synthetic_event_handler(ev.id, &obj);
        self.pending_offer.set(obj.id, obj);
        Ok(())
    }

    fn selection(&self, ev: Selection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.selection.push(self.take_offer(ev.id)?);
        Ok(())
    }

    fn finished(&self, _ev: Finished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn primary_selection(&self, ev: PrimarySelection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.primary_selection.push(self.take_offer(ev.id)?);
        Ok(())
    }
}

impl TestClient {
    pub fn get_data_control_device(&self, seat: &TestSeat) -> Rc<TestDataControlDevice> {
        let client = &self.client;
        let id = client.send_zwlr_data_control_manager_v1_get_data_device(seat.id);
        let dev = Rc::new(TestDataControlDevice {
            id,
            client: client.clone(),
            pending_offer: Default::default(),
            selection: Default::default(),
            primary_selection: Default::default(),
        });
        client.set_synthetic_event_handler(id, &dev);
        dev
    }
}

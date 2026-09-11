use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::wire::WlDataOfferId;
use crate::wire::wl_data_offer::*;
use std::rc::Rc;

pub struct TestDataOffer {
    pub id: WlDataOfferId,
    pub client: Rc<Client>,
}

impl TestDataOffer {
    pub fn destroy(&self) {
        self.client.send_wl_data_offer_destroy(self.id);
    }
}

synthetic_event_handler!(TestDataOffer);

impl WlDataOfferEventHandler for TestDataOffer {
    type Error = TestErrorError;

    fn offer(&self, _ev: Offer<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn source_actions(&self, _ev: SourceActions, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn action(&self, _ev: Action, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

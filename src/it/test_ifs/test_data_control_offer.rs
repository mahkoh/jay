use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::utils::bhash::BHashSet;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use crate::wire::ZwlrDataControlOfferV1Id;
use crate::wire::zwlr_data_control_offer_v1::*;
use std::cell::RefCell;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct TestDataControlOffer {
    pub id: ZwlrDataControlOfferV1Id,
    pub client: Rc<Client>,
    pub offers: RefCell<BHashSet<String>>,
}

impl TestDataControlOffer {
    pub fn receive(&self, mime_type: &str) -> TestResult<Rc<OwnedFd>> {
        let Pipe { read, write } = pipe()?;
        self.client
            .send_zwlr_data_control_offer_v1_receive(self.id, mime_type, &Rc::new(write));
        Ok(Rc::new(read))
    }
}

synthetic_event_handler!(TestDataControlOffer);

impl ZwlrDataControlOfferV1EventHandler for TestDataControlOffer {
    type Error = TestErrorError;

    fn offer(&self, ev: Offer<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.offers.borrow_mut().insert(ev.mime_type.to_string());
        Ok(())
    }
}

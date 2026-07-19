use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::utils::bhash::BHashSet;
use crate::utils::buffd::MsgParser;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use crate::wire::ZwlrDataControlOfferV1Id;
use crate::wire::zwlr_data_control_offer_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct TestDataControlOffer {
    pub id: ZwlrDataControlOfferV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub offers: RefCell<BHashSet<String>>,
}

impl TestDataControlOffer {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn receive(&self, mime_type: &str) -> TestResult<Rc<OwnedFd>> {
        let Pipe { read, write } = pipe()?;
        self.tran.send(Receive {
            self_id: self.id,
            mime_type,
            fd: Rc::new(write),
        })?;
        Ok(Rc::new(read))
    }

    fn handle_offer(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Offer::parse_full(parser)?;
        self.offers.borrow_mut().insert(ev.mime_type.to_string());
        Ok(())
    }
}

impl Drop for TestDataControlOffer {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDataControlOffer, ZwlrDataControlOfferV1;

    OFFER => handle_offer,
}

impl TestObject for TestDataControlOffer {}

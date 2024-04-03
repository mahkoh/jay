use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{zwlr_data_control_offer_v1::*, ZwlrDataControlOfferV1Id},
    },
    ahash::AHashSet,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    uapi::{c, OwnedFd},
};

pub struct TestDataControlOffer {
    pub id: ZwlrDataControlOfferV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub offers: RefCell<AHashSet<String>>,
}

impl TestDataControlOffer {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn receive(&self, mime_type: &str) -> TestResult<Rc<OwnedFd>> {
        let (read, write) = uapi::pipe2(c::O_CLOEXEC)?;
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

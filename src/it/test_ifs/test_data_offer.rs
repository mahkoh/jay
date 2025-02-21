use {
    crate::{
        it::{
            test_error::TestResult, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{WlDataOfferId, wl_data_offer::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestDataOffer {
    pub id: WlDataOfferId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestDataOffer {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_offer(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Offer::parse_full(parser)?;
        Ok(())
    }

    fn handle_source_actions(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = SourceActions::parse_full(parser)?;
        Ok(())
    }

    fn handle_action(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Action::parse_full(parser)?;
        Ok(())
    }
}

impl Drop for TestDataOffer {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDataOffer, WlDataOffer;

    OFFER => handle_offer,
    SOURCE_ACTIONS => handle_source_actions,
    ACTION => handle_action,
}

impl TestObject for TestDataOffer {}

use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_data_control_offer::TestDataControlOffer;
use crate::it::test_ifs::test_data_control_source::TestDataControlSource;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::test_utils::test_expected_event::TEEH;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::ZwlrDataControlDeviceV1Id;
use crate::wire::ZwlrDataControlOfferV1Id;
use crate::wire::zwlr_data_control_device_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestDataControlDevice {
    pub id: ZwlrDataControlDeviceV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub pending_offer: CopyHashMap<ZwlrDataControlOfferV1Id, Rc<TestDataControlOffer>>,
    pub selection: TEEH<Option<Rc<TestDataControlOffer>>>,
    pub primary_selection: TEEH<Option<Rc<TestDataControlOffer>>>,
}

impl TestDataControlDevice {
    #[expect(dead_code)]
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_selection(&self, source: &TestDataControlSource) -> TestResult {
        self.tran.send(SetSelection {
            self_id: self.id,
            source: source.id,
        })?;
        Ok(())
    }

    #[expect(dead_code)]
    pub fn set_primary_selection(&self, source: &TestDataControlSource) -> TestResult {
        self.tran.send(SetPrimarySelection {
            self_id: self.id,
            source: source.id,
        })?;
        Ok(())
    }

    fn handle_data_offer(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = DataOffer::parse_full(parser)?;
        let obj = Rc::new(TestDataControlOffer {
            id: ev.id,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            offers: Default::default(),
        });
        self.tran.add_obj(obj.clone())?;
        self.pending_offer.set(obj.id, obj);
        Ok(())
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

    fn handle_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Selection::parse_full(parser)?;
        self.selection.push(self.take_offer(ev.id)?);
        Ok(())
    }

    fn handle_primary_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = PrimarySelection::parse_full(parser)?;
        self.primary_selection.push(self.take_offer(ev.id)?);
        Ok(())
    }

    fn handle_finished(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Finished::parse_full(parser)?;
        Ok(())
    }
}

test_object! {
    TestDataControlDevice, ZwlrDataControlDeviceV1;

    DATA_OFFER => handle_data_offer,
    SELECTION => handle_selection,
    FINISHED => handle_finished,
    PRIMARY_SELECTION => handle_primary_selection,
}

impl TestObject for TestDataControlDevice {}

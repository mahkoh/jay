use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{
                test_data_offer::TestDataOffer, test_data_source::TestDataSource,
                test_surface::TestSurface,
            },
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{wl_data_device::*, WlDataDeviceId, WlSurfaceId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestDataDevice {
    pub id: WlDataDeviceId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestDataDevice {
    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Release { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn start_drag(
        &self,
        source: &TestDataSource,
        origin: &TestSurface,
        icon: Option<&TestSurface>,
        serial: u32,
    ) -> TestResult {
        self.tran.send(StartDrag {
            self_id: self.id,
            source: source.id,
            origin: origin.id,
            icon: icon.map(|i| i.id).unwrap_or(WlSurfaceId::NONE),
            serial,
        })?;
        Ok(())
    }

    pub fn set_selection(&self, source: &TestDataSource, serial: u32) -> TestResult {
        self.tran.send(SetSelection {
            self_id: self.id,
            source: source.id,
            serial,
        })?;
        Ok(())
    }

    fn handle_data_offer(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let ev = DataOffer::parse_full(parser)?;
        let offer = Rc::new(TestDataOffer {
            id: ev.id,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(offer.clone())?;
        offer.destroy()?;
        Ok(())
    }

    fn handle_enter(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Enter::parse_full(parser)?;
        Ok(())
    }

    fn handle_leave(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Leave::parse_full(parser)?;
        Ok(())
    }

    fn handle_motion(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Motion::parse_full(parser)?;
        Ok(())
    }

    fn handle_drop(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Drop::parse_full(parser)?;
        Ok(())
    }

    fn handle_selection(&self, parser: MsgParser<'_, '_>) -> TestResult {
        let _ev = Selection::parse_full(parser)?;
        Ok(())
    }
}

impl std::ops::Drop for TestDataDevice {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDataDevice, WlDataDevice;

    DATA_OFFER => handle_data_offer,
    ENTER => handle_enter,
    LEAVE => handle_leave,
    MOTION => handle_motion,
    DROP => handle_drop,
    SELECTION => handle_selection,
}

impl TestObject for TestDataDevice {}

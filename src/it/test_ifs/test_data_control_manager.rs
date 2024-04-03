use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{
                test_data_control_device::TestDataControlDevice,
                test_data_control_source::TestDataControlSource, test_seat::TestSeat,
            },
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{zwlr_data_control_manager_v1::*, ZwlrDataControlManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestDataControlManager {
    pub id: ZwlrDataControlManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestDataControlManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    pub fn create_data_source(&self) -> TestResult<Rc<TestDataControlSource>> {
        let obj = Rc::new(TestDataControlSource {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            cancelled: Cell::new(false),
            sends: Default::default(),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(CreateDataSource {
            self_id: self.id,
            id: obj.id,
        })?;
        Ok(obj)
    }

    pub fn get_data_device(&self, seat: &TestSeat) -> TestResult<Rc<TestDataControlDevice>> {
        let obj = Rc::new(TestDataControlDevice {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            pending_offer: Default::default(),
            selection: Default::default(),
            primary_selection: Default::default(),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetDataDevice {
            self_id: self.id,
            id: obj.id,
            seat: seat.id,
        })?;
        Ok(obj)
    }

    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }
}

impl Drop for TestDataControlManager {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestDataControlManager, ZwlrDataControlManagerV1;
}

impl TestObject for TestDataControlManager {}

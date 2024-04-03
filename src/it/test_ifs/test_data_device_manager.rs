use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{
                test_data_device::TestDataDevice, test_data_source::TestDataSource,
                test_seat::TestSeat,
            },
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{wl_data_device_manager::*, WlDataDeviceManagerId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestDataDeviceManager {
    pub id: WlDataDeviceManagerId,
    pub tran: Rc<TestTransport>,
}

impl TestDataDeviceManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn create_data_source(&self) -> TestResult<Rc<TestDataSource>> {
        let data_source = Rc::new(TestDataSource {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            sends: Rc::new(Default::default()),
        });
        self.tran.add_obj(data_source.clone())?;
        self.tran.send(CreateDataSource {
            self_id: self.id,
            id: data_source.id,
        })?;
        Ok(data_source)
    }

    pub fn get_data_device(&self, seat: &TestSeat) -> TestResult<Rc<TestDataDevice>> {
        let data_device = Rc::new(TestDataDevice {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(data_device.clone())?;
        self.tran.send(GetDataDevice {
            self_id: self.id,
            id: data_device.id,
            seat: seat.id,
        })?;
        Ok(data_device)
    }
}

test_object! {
    TestDataDeviceManager, WlDataDeviceManager;
}

impl TestObject for TestDataDeviceManager {}

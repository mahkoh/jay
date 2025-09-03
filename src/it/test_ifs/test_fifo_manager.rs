use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_surface::TestSurface, test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{WpFifoManagerV1Id, WpFifoV1Id, wp_fifo_manager_v1::*, wp_fifo_v1},
    },
    std::rc::Rc,
};

pub struct TestFifoManager {
    pub id: WpFifoManagerV1Id,
    pub tran: Rc<TestTransport>,
}

pub struct TestFifo {
    pub id: WpFifoV1Id,
    pub tran: Rc<TestTransport>,
}

impl TestFifoManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn get_fifo(&self, surface: &TestSurface) -> Result<Rc<TestFifo>, TestError> {
        let obj = Rc::new(TestFifo {
            id: self.tran.id(),
            tran: self.tran.clone(),
        });
        self.tran.send(GetFifo {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(obj)
    }

    #[expect(dead_code)]
    pub fn destroy(&self) -> Result<(), TestError> {
        self.tran.send(Destroy { self_id: self.id })?;
        Ok(())
    }
}

impl TestFifo {
    pub fn set_barrier(&self) -> Result<(), TestError> {
        self.tran.send(wp_fifo_v1::SetBarrier { self_id: self.id })
    }

    pub fn wait_barrier(&self) -> Result<(), TestError> {
        self.tran.send(wp_fifo_v1::WaitBarrier { self_id: self.id })
    }

    #[expect(dead_code)]
    pub fn destroy(&self) -> Result<(), TestError> {
        self.tran.send(wp_fifo_v1::Destroy { self_id: self.id })?;
        Ok(())
    }
}

test_object! {
    TestFifoManager, WpFifoManagerV1;
}

test_object! {
    TestFifo, WpFifoV1;
}

impl TestObject for TestFifoManager {}
impl TestObject for TestFifo {}

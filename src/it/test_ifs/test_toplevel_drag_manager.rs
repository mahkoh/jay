use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{test_data_source::TestDataSource, test_toplevel_drag::TestToplevelDrag},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{xdg_toplevel_drag_manager_v1::*, XdgToplevelDragManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestToplevelDragManager {
    pub id: XdgToplevelDragManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestToplevelDragManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn get_xdg_toplevel_drag(
        &self,
        data_source: &TestDataSource,
    ) -> TestResult<Rc<TestToplevelDrag>> {
        let obj = Rc::new(TestToplevelDrag {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetXdgToplevelDrag {
            self_id: self.id,
            id: obj.id,
            data_source: data_source.id,
        })?;
        Ok(obj)
    }
}

impl Drop for TestToplevelDragManager {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestToplevelDragManager, XdgToplevelDragManagerV1;
}

impl TestObject for TestToplevelDragManager {}

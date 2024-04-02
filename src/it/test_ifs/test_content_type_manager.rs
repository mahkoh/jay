use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{test_content_type::TestContentType, test_surface::TestSurface},
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{wp_content_type_manager_v1::*, WpContentTypeManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestContentTypeManager {
    pub id: WpContentTypeManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestContentTypeManager {
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

    pub fn get_surface_content_type(
        &self,
        surface: &TestSurface,
    ) -> TestResult<Rc<TestContentType>> {
        let obj = Rc::new(TestContentType {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetSurfaceContentType {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        Ok(obj)
    }
}

impl Drop for TestContentTypeManager {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestContentTypeManager, WpContentTypeManagerV1;
}

impl TestObject for TestContentTypeManager {}

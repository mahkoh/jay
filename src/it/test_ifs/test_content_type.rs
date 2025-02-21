use {
    crate::{
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{WpContentTypeV1Id, wp_content_type_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestContentType {
    pub id: WpContentTypeV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestContentType {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_content_type(&self, content_type: u32) -> Result<(), TestError> {
        self.tran.send(SetContentType {
            self_id: self.id,
            content_type,
        })?;
        Ok(())
    }
}

impl Drop for TestContentType {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestContentType, WpContentTypeV1;
}

impl TestObject for TestContentType {}

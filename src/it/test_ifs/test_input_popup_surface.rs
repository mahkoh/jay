use {
    crate::{
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{zwp_input_popup_surface_v2::*, ZwpInputPopupSurfaceV2Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestInputPopupSurface {
    pub id: ZwpInputPopupSurfaceV2Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestInputPopupSurface {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }
}

impl Drop for TestInputPopupSurface {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestInputPopupSurface, ZwpInputPopupSurfaceV2;
}

impl TestObject for TestInputPopupSurface {}

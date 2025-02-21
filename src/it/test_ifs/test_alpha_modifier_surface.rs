use {
    crate::{
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{WpAlphaModifierSurfaceV1Id, wp_alpha_modifier_surface_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestAlphaModifierSurface {
    pub id: WpAlphaModifierSurfaceV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestAlphaModifierSurface {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_multiplier(&self, factor: f64) -> Result<(), TestError> {
        self.tran.send(SetMultiplier {
            self_id: self.id,
            factor: (factor * u32::MAX as f64) as u32,
        })
    }
}

impl Drop for TestAlphaModifierSurface {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestAlphaModifierSurface, WpAlphaModifierSurfaceV1;
}

impl TestObject for TestAlphaModifierSurface {}

use {
    crate::{
        it::{
            test_error::TestResult,
            test_ifs::{
                test_alpha_modifier_surface::TestAlphaModifierSurface, test_surface::TestSurface,
            },
            test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{WpAlphaModifierV1Id, wp_alpha_modifier_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestAlphaModifier {
    pub id: WpAlphaModifierV1Id,
    pub tran: Rc<TestTransport>,
}

impl TestAlphaModifier {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn get_surface(&self, surface: &TestSurface) -> TestResult<Rc<TestAlphaModifierSurface>> {
        let obj = Rc::new(TestAlphaModifierSurface {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(obj.clone())?;
        self.tran.send(GetSurface {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        })?;
        Ok(obj)
    }
}

test_object! {
    TestAlphaModifier, WpAlphaModifierV1;
}

impl TestObject for TestAlphaModifier {}

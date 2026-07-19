use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_ifs::test_xdg_activation_token::TestXdgActivationToken;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::wire::XdgActivationV1Id;
use crate::wire::xdg_activation_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestXdgActivation {
    pub id: XdgActivationV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestXdgActivation {
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

    pub async fn get_token(&self) -> Result<String, TestError> {
        let token = Rc::new(TestXdgActivationToken {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            token: Cell::new(None),
        });
        self.tran.add_obj(token.clone())?;
        self.tran.send(GetActivationToken {
            self_id: self.id,
            id: token.id,
        })?;
        let res = token.commit().await?;
        token.destroy()?;
        Ok(res)
    }

    pub fn activate(&self, tl: &TestSurface, token: &str) -> TestResult {
        self.tran.send(Activate {
            self_id: self.id,
            token,
            surface: tl.id,
        })
    }
}

test_object! {
    TestXdgActivation, XdgActivationV1;
}

impl TestObject for TestXdgActivation {}

impl Drop for TestXdgActivation {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

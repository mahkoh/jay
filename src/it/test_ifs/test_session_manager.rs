use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_session::TestSession, test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{XdgSessionManagerV1Id, xdg_session_manager_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSessionManager {
    pub id: XdgSessionManagerV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestSessionManager {
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

    pub fn get_session(
        &self,
        reason: u32,
        session_id: Option<&str>,
    ) -> Result<Rc<TestSession>, TestError> {
        let id = self.tran.id();
        self.tran.send(GetSession {
            self_id: self.id,
            id,
            reason,
            session_id,
        })?;
        let session = Rc::new(TestSession {
            id,
            tran: self.tran.clone(),
            destroyed: Default::default(),
            result: Default::default(),
            result_waiter: Default::default(),
            replaced: Default::default(),
        });
        self.tran.add_obj(session.clone())?;
        Ok(session)
    }
}

test_object! {
    TestSessionManager, XdgSessionManagerV1;
}

impl TestObject for TestSessionManager {}

impl Drop for TestSessionManager {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

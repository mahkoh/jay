use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_toplevel_session::TestToplevelSession,
            test_object::TestObject, test_transport::TestTransport,
            test_utils::test_window::TestWindow, testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{XdgSessionV1Id, XdgToplevelId, xdg_session_v1::*},
    },
    std::{
        cell::Cell,
        future::poll_fn,
        rc::Rc,
        task::{Poll, Waker},
    },
};

pub struct TestSession {
    pub id: XdgSessionV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub result: Cell<Option<TestSessionResult>>,
    pub result_waiter: Cell<Option<Waker>>,
    pub replaced: Cell<bool>,
}

pub enum TestSessionResult {
    Created(String),
    Restored,
}

impl TestSession {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    #[expect(dead_code)]
    pub fn remove(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Remove { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn add_toplevel(
        &self,
        win: &TestWindow,
        name: &str,
    ) -> Result<Rc<TestToplevelSession>, TestError> {
        self.add_toplevel2(win.tl.server.id, name)
    }

    pub fn add_toplevel2(
        &self,
        toplevel: XdgToplevelId,
        name: &str,
    ) -> Result<Rc<TestToplevelSession>, TestError> {
        let id = self.tran.id();
        self.tran.send(AddToplevel {
            self_id: self.id,
            id,
            toplevel,
            name,
        })?;
        let ts = Rc::new(TestToplevelSession {
            id,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            restored: Cell::new(false),
        });
        self.tran.add_obj(ts.clone())?;
        Ok(ts)
    }

    pub fn restore_toplevel(
        &self,
        win: &TestWindow,
        name: &str,
    ) -> Result<Rc<TestToplevelSession>, TestError> {
        self.restore_toplevel2(win.tl.server.id, name)
    }

    pub fn restore_toplevel2(
        &self,
        toplevel: XdgToplevelId,
        name: &str,
    ) -> Result<Rc<TestToplevelSession>, TestError> {
        let id = self.tran.id();
        self.tran.send(RestoreToplevel {
            self_id: self.id,
            id,
            toplevel,
            name,
        })?;
        let ts = Rc::new(TestToplevelSession {
            id,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            restored: Cell::new(false),
        });
        self.tran.add_obj(ts.clone())?;
        Ok(ts)
    }

    #[expect(dead_code)]
    pub fn remove_toplevel(&self, name: &str) -> Result<(), TestError> {
        self.tran.send(RemoveToplevel {
            self_id: self.id,
            name,
        })?;
        Ok(())
    }

    pub async fn result_created(&self) -> Result<String, TestError> {
        let res = self.result().await;
        match res {
            TestSessionResult::Created(id) => Ok(id),
            TestSessionResult::Restored => bail!("Session was restored instead of created"),
        }
    }

    #[expect(dead_code)]
    pub async fn result_restored(&self) -> Result<(), TestError> {
        let res = self.result().await;
        match res {
            TestSessionResult::Created(id) => {
                bail!("Session was created ({id}) instead of restored")
            }
            TestSessionResult::Restored => Ok(()),
        }
    }

    pub async fn result(&self) -> TestSessionResult {
        poll_fn(|ctx| {
            if let Some(res) = self.result.take() {
                return Poll::Ready(res);
            }
            self.result_waiter.set(Some(ctx.waker().clone()));
            Poll::Pending
        })
        .await
    }

    fn set_result(&self, result: TestSessionResult) {
        self.result.set(Some(result));
        if let Some(waker) = self.result_waiter.take() {
            waker.wake();
        }
    }

    fn handle_created(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Created::parse_full(parser)?;
        self.set_result(TestSessionResult::Created(ev.session_id.to_string()));
        Ok(())
    }

    fn handle_restored(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Restored::parse_full(parser)?;
        self.set_result(TestSessionResult::Restored);
        Ok(())
    }

    fn handle_replaced(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Replaced::parse_full(parser)?;
        self.replaced.set(true);
        Ok(())
    }
}

test_object! {
    TestSession, XdgSessionV1;

    CREATED => handle_created,
    RESTORED => handle_restored,
    REPLACED => handle_replaced,
}

impl TestObject for TestSession {}

impl Drop for TestSession {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestError;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_toplevel_session::TestToplevelSession;
use crate::it::test_utils::test_window::TestWindow;
use crate::wire::XdgSessionV1Id;
use crate::wire::XdgToplevelId;
use crate::wire::XdgToplevelSessionV1Id;
use crate::wire::xdg_session_v1::*;
use std::cell::Cell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;
use std::task::Waker;

pub struct TestSession {
    pub id: XdgSessionV1Id,
    pub client: Rc<Client>,
    pub result: Cell<Option<TestSessionResult>>,
    pub result_waiter: Cell<Option<Waker>>,
    pub replaced: Cell<bool>,
}

pub enum TestSessionResult {
    Created(String),
    Restored,
}

impl TestSession {
    #[expect(unused)]
    pub fn remove(&self) {
        self.client.send_xdg_session_v1_remove(self.id);
    }

    pub fn add_toplevel(&self, win: &TestWindow, name: &str) -> Rc<TestToplevelSession> {
        self.add_toplevel2(win.tl.server.id, name)
    }

    fn add_toplevel2(&self, toplevel: XdgToplevelId, name: &str) -> Rc<TestToplevelSession> {
        let id = self
            .client
            .send_xdg_session_v1_add_toplevel(self.id, toplevel, name);
        self.toplevel_session(id)
    }

    pub fn restore_toplevel(&self, win: &TestWindow, name: &str) -> Rc<TestToplevelSession> {
        self.restore_toplevel2(win.tl.server.id, name)
    }

    fn restore_toplevel2(&self, toplevel: XdgToplevelId, name: &str) -> Rc<TestToplevelSession> {
        let id = self
            .client
            .send_xdg_session_v1_restore_toplevel(self.id, toplevel, name);
        self.toplevel_session(id)
    }

    fn toplevel_session(&self, id: XdgToplevelSessionV1Id) -> Rc<TestToplevelSession> {
        let ts = Rc::new(TestToplevelSession {
            id,
            client: self.client.clone(),
            restored: Cell::new(false),
        });
        self.client.set_synthetic_event_handler(id, &ts);
        ts
    }

    #[expect(unused)]
    pub fn remove_toplevel(&self, name: &str) {
        self.client
            .send_xdg_session_v1_remove_toplevel(self.id, name);
    }

    pub async fn result_created(&self) -> Result<String, TestError> {
        let res = self.result().await;
        match res {
            TestSessionResult::Created(id) => Ok(id),
            TestSessionResult::Restored => bail!("Session was restored instead of created"),
        }
    }

    #[expect(unused)]
    pub async fn result_restored(&self) -> Result<(), TestError> {
        let res = self.result().await;
        match res {
            TestSessionResult::Created(id) => {
                bail!("Session was created ({id}) instead of restored")
            }
            TestSessionResult::Restored => Ok(()),
        }
    }

    async fn result(&self) -> TestSessionResult {
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
}

synthetic_event_handler!(TestSession);

impl XdgSessionV1EventHandler for TestSession {
    type Error = TestErrorError;

    fn created(&self, ev: Created<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_result(TestSessionResult::Created(ev.session_id.to_string()));
        Ok(())
    }

    fn restored(&self, _ev: Restored, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_result(TestSessionResult::Restored);
        Ok(())
    }

    fn replaced(&self, _ev: Replaced, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.replaced.set(true);
        Ok(())
    }
}

impl TestClient {
    pub fn get_session(&self, reason: u32, session_id: Option<&str>) -> Rc<TestSession> {
        let client = &self.client;
        let id = client.send_xdg_session_manager_v1_get_session(reason, session_id);
        let session = Rc::new(TestSession {
            id,
            client: client.clone(),
            result: Default::default(),
            result_waiter: Default::default(),
            replaced: Default::default(),
        });
        client.set_synthetic_event_handler(id, &session);
        session
    }
}

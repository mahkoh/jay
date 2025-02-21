use {
    crate::{
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{XdgActivationTokenV1Id, xdg_activation_token_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestXdgActivationToken {
    pub id: XdgActivationTokenV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub token: Cell<Option<String>>,
}

impl TestXdgActivationToken {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub async fn commit(&self) -> Result<String, TestError> {
        self.tran.send(Commit { self_id: self.id })?;
        self.tran.sync().await;
        match self.token.take() {
            Some(t) => Ok(t),
            _ => bail!("Server did not send a token"),
        }
    }

    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Done::parse_full(parser)?;
        self.token.set(Some(ev.token.to_string()));
        Ok(())
    }
}

test_object! {
    TestXdgActivationToken, XdgActivationTokenV1;

    DONE => handle_done,
}

impl TestObject for TestXdgActivationToken {}

impl Drop for TestXdgActivationToken {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

use {
    crate::{
        client::ClientId,
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{
            jay_compositor::{self, *},
            JayCompositorId,
        },
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestJayCompositor {
    pub id: JayCompositorId,
    pub transport: Rc<TestTransport>,
    pub client_id: Cell<Option<ClientId>>,
}

impl TestJayCompositor {
    pub async fn get_client_id(&self) -> Result<ClientId, TestError> {
        if self.client_id.get().is_none() {
            self.transport.send(GetClientId { self_id: self.id });
        }
        self.transport.sync().await;
        match self.client_id.get() {
            Some(c) => Ok(c),
            _ => bail!("Compositor did not send a client id"),
        }
    }

    fn handle_client_id(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = jay_compositor::ClientId::parse_full(parser)?;
        self.client_id.set(Some(ClientId::from_raw(ev.client_id)));
        Ok(())
    }
}

test_object! {
    TestJayCompositor, JayCompositor;

    CLIENT_ID => handle_client_id,
}

impl TestObject for TestJayCompositor {}

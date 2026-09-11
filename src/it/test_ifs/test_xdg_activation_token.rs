use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestError;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::wire::XdgActivationTokenV1Id;
use crate::wire::xdg_activation_token_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestXdgActivationToken {
    pub id: XdgActivationTokenV1Id,
    pub client: Rc<Client>,
    pub token: Cell<Option<String>>,
}

impl TestXdgActivationToken {
    pub fn destroy(&self) {
        self.client.send_xdg_activation_token_v1_destroy(self.id);
    }

    pub async fn commit(&self) -> Result<String, TestError> {
        self.client.send_xdg_activation_token_v1_commit(self.id);
        self.client.sync().await;
        match self.token.take() {
            Some(t) => Ok(t),
            _ => bail!("Server did not send a token"),
        }
    }
}

synthetic_event_handler!(TestXdgActivationToken);

impl XdgActivationTokenV1EventHandler for TestXdgActivationToken {
    type Error = TestErrorError;

    fn done(&self, ev: Done<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.token.set(Some(ev.token.to_string()));
        Ok(())
    }
}

impl TestClient {
    pub async fn get_activation_token(&self) -> Result<String, TestError> {
        let client = &self.client;
        let id = client.send_xdg_activation_v1_get_activation_token();
        let token = Rc::new(TestXdgActivationToken {
            id,
            client: client.clone(),
            token: Cell::new(None),
        });
        client.set_synthetic_event_handler(id, &token);
        let res = token.commit().await?;
        token.destroy();
        Ok(res)
    }

    pub fn activate(&self, surface: &TestSurface, token: &str) {
        self.client
            .send_xdg_activation_v1_activate(token, surface.id);
    }
}

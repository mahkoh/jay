use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_ifs::test_data_source::TestDataSource;
use crate::it::test_ifs::test_xdg_toplevel::TestXdgToplevel;
use crate::wire::XdgToplevelDragV1Id;
use std::rc::Rc;

pub struct TestToplevelDrag {
    pub id: XdgToplevelDragV1Id,
    pub client: Rc<Client>,
}

impl TestToplevelDrag {
    pub fn attach(&self, toplevel: &TestXdgToplevel, x_offset: i32, y_offset: i32) {
        self.client
            .send_xdg_toplevel_drag_v1_attach(self.id, toplevel.core.id, x_offset, y_offset);
    }
}

impl TestClient {
    pub fn get_xdg_toplevel_drag(&self, data_source: &TestDataSource) -> Rc<TestToplevelDrag> {
        let id = self
            .client
            .send_xdg_toplevel_drag_manager_v1_get_xdg_toplevel_drag(data_source.id);
        Rc::new(TestToplevelDrag {
            id,
            client: self.client.clone(),
        })
    }
}

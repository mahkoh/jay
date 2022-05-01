use {
    crate::{
        client::Client,
        it::{
            test_ifs::{
                test_compositor::TestCompositor, test_jay_compositor::TestJayCompositor,
                test_registry::TestRegistry, test_shm::TestShm,
            },
            test_transport::TestTransport,
            testrun::TestRun,
        },
    },
    std::rc::Rc,
};

pub struct TestClient {
    pub run: Rc<TestRun>,
    pub server: Rc<Client>,
    pub transport: Rc<TestTransport>,
    pub registry: Rc<TestRegistry>,
    pub jc: Rc<TestJayCompositor>,
    pub comp: Rc<TestCompositor>,
    pub shm: Rc<TestShm>,
}

impl TestClient {
    pub fn error(&self, msg: &str) {
        self.transport.error(msg)
    }

    pub async fn sync(self: &Rc<Self>) {
        self.transport.sync().await
    }
}

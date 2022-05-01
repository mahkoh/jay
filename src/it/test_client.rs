use {
    crate::{
        cli::screenshot::buf_to_qoi,
        client::Client,
        it::{
            test_error::TestError,
            test_ifs::{
                test_compositor::TestCompositor, test_jay_compositor::TestJayCompositor,
                test_registry::TestRegistry, test_shm::TestShm, test_xdg_base::TestXdgWmBase,
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
    pub tran: Rc<TestTransport>,
    pub registry: Rc<TestRegistry>,
    pub jc: Rc<TestJayCompositor>,
    pub comp: Rc<TestCompositor>,
    pub shm: Rc<TestShm>,
    pub xdg: Rc<TestXdgWmBase>,
}

impl TestClient {
    pub fn error(&self, msg: &str) {
        self.tran.error(msg)
    }

    pub async fn sync(self: &Rc<Self>) {
        self.tran.sync().await
    }

    pub async fn take_screenshot(&self) -> Result<Vec<u8>, TestError> {
        let dmabuf = self.jc.take_screenshot().await?;
        let qoi = buf_to_qoi(&dmabuf);
        Ok(qoi)
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.tran.kill();
    }
}

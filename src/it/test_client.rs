use {
    crate::{
        cli::screenshot::buf_to_qoi,
        client::Client,
        format::ARGB8888,
        it::{
            test_error::TestError,
            test_ifs::{
                test_compositor::TestCompositor, test_jay_compositor::TestJayCompositor,
                test_registry::TestRegistry, test_shm::TestShm, test_xdg_base::TestXdgWmBase,
            },
            test_transport::TestTransport,
            test_utils::test_window::TestWindow,
            testrun::TestRun,
        },
        theme::Color,
        utils::clonecell::CloneCell,
    },
    std::{cell::Cell, rc::Rc},
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
    #[allow(dead_code)]
    pub fn error(&self, msg: &str) {
        self.tran.error(msg)
    }

    pub async fn sync(&self) {
        self.tran.sync().await
    }

    #[allow(dead_code)]
    pub async fn take_screenshot(&self) -> Result<Vec<u8>, TestError> {
        let dmabuf = self.jc.take_screenshot().await?;
        let qoi = buf_to_qoi(&dmabuf);
        Ok(qoi)
    }

    pub async fn create_window(&self) -> Result<Rc<TestWindow>, TestError> {
        let surface = self.comp.create_surface().await?;
        let shm = self.shm.create_pool(0)?;
        let buffer = shm.create_buffer(0, 0, 0, 0, ARGB8888)?;
        let xdg = self.xdg.create_xdg_surface(surface.id).await?;
        let tl = xdg.create_toplevel().await?;
        surface.commit();
        self.sync().await;
        Ok(Rc::new(TestWindow {
            surface,
            xdg,
            tl,
            shm,
            buffer: CloneCell::new(buffer),
            color: Cell::new(Color::from_rgba_straight(0, 0, 0, 0)),
        }))
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.tran.kill();
    }
}

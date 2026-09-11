use crate::client::Client;
use crate::format::Format;
use crate::it::test_client::TestClient;
use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_buffer::TestBuffer;
use crate::it::test_ifs::test_shm_buffer::TestShmBuffer;
use crate::it::test_mem::TestMem;
use crate::utils::clonecell::CloneCell;
use crate::wire::WlShmPoolId;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestShmPool {
    pub id: WlShmPoolId,
    pub client: Rc<Client>,
    pub mem: CloneCell<Rc<TestMem>>,
}

impl TestShmPool {
    pub fn create_buffer(
        &self,
        offset: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &Format,
    ) -> Result<Rc<TestShmBuffer>, TestError> {
        let size = (height * stride) as usize;
        let start = offset as usize;
        let end = start + size;
        let mem = self.mem.get();
        if end > mem.len() {
            bail!("Out-of-bounds buffer");
        }
        let client = &self.client;
        let id = client.send_wl_shm_pool_create_buffer(
            self.id,
            offset,
            width,
            height,
            stride,
            format.wl_id.unwrap_or(format.drm),
        );
        let buffer = Rc::new(TestBuffer {
            id,
            released: Cell::new(true),
        });
        client.set_synthetic_event_handler(id, &buffer);
        Ok(Rc::new(TestShmBuffer {
            buffer,
            range: start..end,
            mem,
        }))
    }

    #[expect(unused)]
    pub fn resize(&self, size: usize) -> Result<(), TestError> {
        let mem = self.mem.get().grow(size)?;
        self.mem.set(mem);
        self.client.send_wl_shm_pool_resize(self.id, size as _);
        Ok(())
    }
}

impl TestClient {
    pub fn create_shm_pool(&self, size: usize) -> TestResult<Rc<TestShmPool>> {
        let mem = TestMem::new(size)?;
        let id = self.client.send_wl_shm_create_pool(&mem.fd, size as _);
        Ok(Rc::new(TestShmPool {
            id,
            client: self.client.clone(),
            mem: CloneCell::new(mem),
        }))
    }
}

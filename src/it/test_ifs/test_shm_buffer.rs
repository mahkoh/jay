use {
    crate::{
        it::{
            test_error::TestError, test_mem::TestMem, test_object::TestObject,
            test_transport::TestTransport, testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{wl_buffer::*, WlBufferId},
    },
    std::{
        cell::Cell,
        ops::{Deref, Range},
        rc::Rc,
    },
};

pub struct TestShmBuffer {
    pub id: WlBufferId,
    pub transport: Rc<TestTransport>,
    pub range: Range<usize>,
    pub mem: Rc<TestMem>,
    pub released: Cell<bool>,
    pub destroyed: Cell<bool>,
}

impl TestShmBuffer {
    pub fn destroy(&self) {
        if self.destroyed.replace(true) {
            return;
        }
        self.transport.send(Destroy { self_id: self.id });
    }

    fn handle_release(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Release::parse_full(parser)?;
        self.released.set(true);
        Ok(())
    }
}

impl Deref for TestShmBuffer {
    type Target = [Cell<u8>];

    fn deref(&self) -> &Self::Target {
        &self.mem[self.range.clone()]
    }
}

impl Drop for TestShmBuffer {
    fn drop(&mut self) {
        self.destroy();
    }
}

test_object! {
    TestShmBuffer, WlBuffer;

    RELEASE => handle_release,
}

impl TestObject for TestShmBuffer {}

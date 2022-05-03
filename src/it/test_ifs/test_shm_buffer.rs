use {
    crate::{
        it::{
            test_error::TestError,
            test_mem::TestMem,
            test_object::{Deleted, TestObject},
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        theme::Color,
        utils::{buffd::MsgParser, windows::WindowsExt},
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
    pub tran: Rc<TestTransport>,
    pub range: Range<usize>,
    pub mem: Rc<TestMem>,
    pub released: Cell<bool>,
    pub destroyed: Cell<bool>,
    pub deleted: Deleted,
}

impl TestShmBuffer {
    pub fn fill(&self, color: Color) {
        let [cr, cg, cb, ca] = color.to_rgba_premultiplied();
        for [b, g, r, a] in self.deref().array_chunks_ext::<4>() {
            r.set(cr);
            g.set(cg);
            b.set(cb);
            a.set(ca);
        }
    }

    pub fn destroy(&self) -> Result<(), TestError> {
        if self.destroyed.replace(true) {
            return Ok(());
        }
        self.deleted.check()?;
        self.tran.send(Destroy { self_id: self.id });
        Ok(())
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
        let _ = self.destroy();
    }
}

test_object! {
    TestShmBuffer, WlBuffer;

    RELEASE => handle_release,
}

impl TestObject for TestShmBuffer {}

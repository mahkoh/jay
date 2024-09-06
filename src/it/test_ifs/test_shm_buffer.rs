use {
    crate::{
        it::{test_ifs::test_buffer::TestBuffer, test_mem::TestMem},
        theme::Color,
        utils::windows::WindowsExt,
    },
    std::{
        cell::Cell,
        ops::{Deref, Range},
        rc::Rc,
    },
};

pub struct TestShmBuffer {
    pub buffer: Rc<TestBuffer>,
    pub range: Range<usize>,
    pub mem: Rc<TestMem>,
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
}

impl Deref for TestShmBuffer {
    type Target = [Cell<u8>];

    fn deref(&self) -> &Self::Target {
        &self.mem[self.range.clone()]
    }
}

use crate::it::test_ifs::test_buffer::TestBuffer;
use crate::it::test_mem::TestMem;
use crate::theme::Color;
use crate::utils::windows::WindowsExt;
use std::cell::Cell;
use std::ops::Deref;
use std::ops::Range;
use std::rc::Rc;

pub struct TestShmBuffer {
    pub buffer: Rc<TestBuffer>,
    pub range: Range<usize>,
    pub mem: Rc<TestMem>,
}

impl TestShmBuffer {
    pub fn fill(&self, color: Color) {
        let [cr, cg, cb, ca] = color.to_srgba_premultiplied();
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

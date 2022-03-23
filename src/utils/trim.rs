pub trait AsciiTrim {
    fn trim(&self) -> &[u8];
    fn trim_start(&self) -> &[u8];
    fn trim_end(&self) -> &[u8];
}

impl AsciiTrim for [u8] {
    fn trim(&self) -> &[u8] {
        self.trim_start().trim_end()
    }

    fn trim_start(&self) -> &[u8] {
        let mut s = self;
        while let Some((b, r)) = s.split_first() {
            if !matches!(*b, b' ' | b'\t' | b'\n') {
                break;
            }
            s = r;
        }
        s
    }

    fn trim_end(&self) -> &[u8] {
        let mut s = self;
        while let Some((b, r)) = s.split_last() {
            if !matches!(*b, b' ' | b'\t' | b'\n') {
                break;
            }
            s = r;
        }
        s
    }
}

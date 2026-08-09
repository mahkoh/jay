use crate::utils::spaces::Spaces;
use crate::utils::spaces::spaces;
use std::fmt::Display;
use std::fmt::Formatter;

pub struct Indent {
    n: usize,
    sp: Spaces,
}

impl Indent {
    pub fn push(&self) -> Self {
        let n = self.n + 4;
        Self { n, sp: spaces(n) }
    }
}

impl Default for Indent {
    fn default() -> Self {
        Self {
            n: 0,
            sp: spaces(0),
        }
    }
}

impl Display for Indent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.sp)
    }
}

macro_rules! define_xn {
    ($xn:ident) => {
        let $xn = &crate::indent::Indent::default();
    };
}

macro_rules! push_xn {
    ($xn:ident) => {
        let $xn = &$xn.push();
    };
}

macro_rules! define_w {
    ($f:expr, $w:ident, $wl:ident) => {
        define_w!($f, $w, $wl, $);
    };
    ($f:expr, $w:ident, $wl:ident, $dol:tt) => {
        #[allow(unused_macros)]
        macro_rules! $w {
            ($dol($dol tt:tt)*) => {
                write!($f, $dol($dol tt)*)?
            };
        }
        #[allow(unused_macros)]
        macro_rules! $wl {
            ($dol($dol tt:tt)*) => {
                writeln!($f, $dol($dol tt)*)?
            };
        }
    };
}

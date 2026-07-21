use arrayvec::ArrayVec;
pub use generated::*;
use linearize::LinearizeExt;
use linearize::Linearized;
use linearize::StaticCopyMap;
use linearize::StaticMap;
pub use matcher::*;
pub use types::*;

#[rustfmt::skip]
mod generated;
mod matcher;
mod types;

#[derive(Copy, Clone, Debug)]
#[repr(Rust, packed)]
pub struct Path {
    pub flags: PathFlags,
    pl_lo: PlLoTy,
    pl_len: PlLenTy,
    cl_lo: ClLoTy,
    cl_len: ClLenTy,
}

macro_rules! get {
    ($($name:ident, $ty:ty, $max_len:expr, $lo:ident, $len:ident, $e:expr, $l:expr;)*) => {
        impl Path {
            $(
                pub fn $name(self, res: &mut ArrayVec<$ty, $max_len>) {
                    res.clear();
                    let lo = self.$lo as usize;
                    let hi = lo + self.$len as usize;
                    for i in lo..hi {
                        unsafe {
                            res.push_unchecked(*$e.get_unchecked(*$l.get_unchecked(i) as usize));
                        }
                    }
                }
            )*
        }

        #[test]
        fn in_bounds() {
            $({
                for path in XL {
                    let lo = path.$lo as usize;
                    let hi = lo + path.$len as usize;
                    assert!((path.$len as usize) <= $max_len);
                    assert!(lo <= hi);
                    assert!(hi <= $l.len());
                }
                for l in $l {
                    assert!((l as usize) < $e.len());
                }
            })*
        }
    };
}

get! {
    plane, PlaneOpKind, MAX_PLANE_PATH_LEN, pl_lo, pl_len, P, PL;
    crtc, CrtcOpKind, MAX_CRTC_PATH_LEN, cl_lo, cl_len, C, CL;
}

#[derive(Debug)]
pub struct Filter {
    filter: StaticMap<Criteria, Box<[MatcherBits]>>,
}

impl Filter {
    pub fn contains(&self, criteria: Linearized<Criteria>, path_idx: usize) -> bool {
        const BITS: usize = MatcherBits::BITS as usize;
        let idx = path_idx / BITS;
        let offset = path_idx % BITS;
        ((self.filter[criteria][idx] >> offset) & 1) == 1
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            filter: PATHS.into_static_map().map_values(|v| {
                vec![0; v.len().div_ceil(MatcherBits::BITS as _)].into_boxed_slice()
            }),
        }
    }
}

pub fn create_filter<T>(
    all: &mut Filter,
    names: &[(T, bool)],
    matchers: &StaticCopyMap<Criteria, &'static dyn Matcher<T>>,
) -> Filter {
    let mut res = Filter::default();
    for c in Criteria::variants() {
        let c = c.linearized();
        let res = &mut res.filter[c];
        let all = &mut all.filter[c];
        let matcher = matchers[c];
        matcher.find(names, res);
        for i in 0..res.len() {
            all[i] |= res[i];
        }
    }
    res
}

// #[test]
// fn print_paths() {
//     for (k, v) in &PATHS {
//         println!("{k:?}");
//         for p in *v {
//             let mut plane = ArrayVec::new();
//             p.plane(&mut plane);
//             let mut crtc = ArrayVec::new();
//             p.crtc(&mut crtc);
//             println!("  - {plane:?}");
//             println!("    {crtc:?}");
//         }
//     }
// }

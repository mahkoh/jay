use {
    crate::{
        rect::{Container, NoTag, RectRaw, Tag},
        windows::WindowsExt,
    },
    std::{cmp::Ordering, collections::BinaryHeap, marker::PhantomData, mem, ops::Deref},
};

pub fn union(left: &Container, right: &Container) -> Container {
    op::<_, _, _, Union>(left, right)
}

pub fn subtract(left: &Container, right: &Container) -> Container {
    op::<_, _, _, Subtract>(left, right)
}

pub fn intersect(left: &Container, right: &Container) -> Container {
    op::<_, _, _, Intersect<NoTag>>(left, right)
}

pub fn intersect_tagged(left: &Container<u32>, right: &Container) -> Container<u32> {
    op::<_, _, _, Intersect<u32>>(left, right)
}

struct Bands<'a, T>
where
    T: Tag,
{
    rects: &'a [RectRaw<T>],
}

#[derive(Copy, Clone)]
struct Band<'a, T>
where
    T: Tag,
{
    rects: &'a [RectRaw<T>],
    y1: i32,
    y2: i32,
}

impl<'a, T> Band<'a, T>
where
    T: Tag,
{
    fn can_merge_with(&self, next: &Band<'_, T>) -> bool {
        next.rects.len() == self.rects.len()
            && next.y1 == self.y2
            && next
                .rects
                .iter()
                .zip(self.rects.iter())
                .all(|(a, b)| (a.x1, a.x2, a.tag) == (b.x1, b.x2, b.tag))
    }
}

impl<'a, T> Iterator for Bands<'a, T>
where
    T: Tag,
{
    type Item = Band<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rects.is_empty() {
            return None;
        }
        let y1 = self.rects[0].y1;
        let y2 = self.rects[0].y2;
        for (pos, rect) in self.rects[1..].iter().enumerate() {
            if rect.y1 != y1 {
                let (res, rects) = self.rects.split_at(pos + 1);
                self.rects = rects;
                return Some(Band { rects: res, y1, y2 });
            }
        }
        Some(Band {
            rects: mem::replace(&mut self.rects, &[]),
            y1,
            y2,
        })
    }
}

#[inline]
pub fn extents<T>(a: &[RectRaw<T>]) -> RectRaw
where
    T: Tag,
{
    let mut a = a.iter();
    let mut res = match a.next() {
        Some(a) => *a,
        _ => return RectRaw::default(),
    };
    for a in a {
        res.x1 = res.x1.min(a.x1);
        res.y1 = res.y1.min(a.y1);
        res.x2 = res.x2.max(a.x2);
        res.y2 = res.y2.max(a.y2);
    }
    RectRaw {
        x1: res.x1,
        y1: res.y1,
        x2: res.x2,
        y2: res.y2,
        tag: NoTag,
    }
}

fn op<T1, T2, T3, O: Op<T1, T2, T3>>(a: &[RectRaw<T2>], b: &[RectRaw<T3>]) -> Container<T1>
where
    T1: Tag,
    T2: Tag,
    T3: Tag,
{
    let mut res = Container::new();

    let mut prev_band_y2 = 0;
    let mut prev_band_start = 0;
    let mut cur_band_start;

    let mut a_bands = Bands { rects: a };
    let mut b_bands = Bands { rects: b };

    let mut a_opt = a_bands.next();
    let mut b_opt = b_bands.next();

    macro_rules! fixup_new_band {
        ($y1:expr, $y2:expr) => {{
            if prev_band_y2 != $y1 || !coalesce(&mut res, prev_band_start, cur_band_start, $y2) {
                prev_band_start = cur_band_start;
            }
            prev_band_y2 = $y2;
        }};
    }

    macro_rules! append_nonoverlapping {
        ($append_opt:expr, $map:ident, $a:expr, $a_opt:expr, $a_bands:expr, $b:expr) => {{
            if $append_opt {
                let y2 = $a.y2.min($b.y1);
                cur_band_start = res.len();
                res.reserve($a.rects.len());
                for rect in $a.rects {
                    res.push(RectRaw {
                        x1: rect.x1,
                        y1: $a.y1,
                        x2: rect.x2,
                        y2,
                        tag: O::$map(rect.tag),
                    });
                }
                fixup_new_band!($a.y1, y2);
            }
            if $a.y2 <= $b.y1 {
                $a_opt = $a_bands.next();
            } else {
                $a.y1 = $b.y1;
            }
        }};
    }

    while let (Some(a), Some(b)) = (&mut a_opt, &mut b_opt) {
        if a.y1 < b.y1 {
            append_nonoverlapping!(O::APPEND_NON_A, map_t2_to_t1, a, a_opt, a_bands, b);
        } else if b.y1 < a.y1 {
            append_nonoverlapping!(O::APPEND_NON_B, map_t3_to_t1, b, b_opt, b_bands, a);
        } else {
            let y2 = a.y2.min(b.y2);
            cur_band_start = res.len();
            O::handle_band(&mut res, a.rects, b.rects, a.y1, y2);
            if res.len() > cur_band_start {
                fixup_new_band!(a.y1, y2);
            }
            if a.y2 == y2 {
                a_opt = a_bands.next();
            } else {
                a.y1 = y2;
            }
            if b.y2 == y2 {
                b_opt = b_bands.next();
            } else {
                b.y1 = y2;
            }
        }
    }

    macro_rules! push_trailing {
        ($a_opt:expr, $a_bands:expr, $map:ident) => {{
            while let Some(a) = $a_opt {
                cur_band_start = res.len();
                res.reserve(a.rects.len());
                for rect in a.rects {
                    res.push(RectRaw {
                        x1: rect.x1,
                        y1: a.y1,
                        x2: rect.x2,
                        y2: a.y2,
                        tag: O::$map(rect.tag),
                    });
                }
                fixup_new_band!(a.y1, a.y2);
                $a_opt = $a_bands.next();
            }
        }};
    }

    if O::APPEND_NON_A {
        push_trailing!(a_opt, a_bands, map_t2_to_t1);
    }

    if O::APPEND_NON_B {
        push_trailing!(b_opt, b_bands, map_t3_to_t1);
    }

    res.shrink_to_fit();
    res
}

fn coalesce<T>(new: &mut Container<T>, a: usize, b: usize, y2: i32) -> bool
where
    T: Tag,
{
    if new.len() - b != b - a {
        return false;
    }
    let slice_a = &new[a..b];
    let slice_b = &new[b..];
    for (a, b) in slice_a.iter().zip(slice_b.iter()) {
        if (a.x1, a.x2, a.tag) != (b.x1, b.x2, b.tag) {
            return false;
        }
    }
    for rect in &mut new[a..b] {
        rect.y2 = y2;
    }
    new.truncate(b);
    true
}

trait Op<T1, T2, T3>
where
    T1: Tag,
    T2: Tag,
    T3: Tag,
{
    const APPEND_NON_A: bool;
    const APPEND_NON_B: bool;

    fn handle_band(new: &mut Container<T1>, a: &[RectRaw<T2>], b: &[RectRaw<T3>], y1: i32, y2: i32);

    fn map_t2_to_t1(tag: T2) -> T1;

    fn map_t3_to_t1(tag: T3) -> T1;
}

struct Union;

impl Op<NoTag, NoTag, NoTag> for Union {
    const APPEND_NON_A: bool = true;
    const APPEND_NON_B: bool = true;

    fn handle_band(new: &mut Container, mut a: &[RectRaw], mut b: &[RectRaw], y1: i32, y2: i32) {
        let mut x1;
        let mut x2;

        macro_rules! push {
            () => {
                new.push(RectRaw {
                    x1,
                    y1,
                    x2,
                    y2,
                    tag: NoTag,
                });
            };
        }

        macro_rules! merge {
            ($r:expr) => {
                if $r.x1 <= x2 {
                    if $r.x2 > x2 {
                        x2 = $r.x2;
                    }
                } else {
                    push!();
                    x1 = $r.x1;
                    x2 = $r.x2;
                }
            };
        }

        if a[0].x1 < b[0].x1 {
            x1 = a[0].x1;
            x2 = a[0].x2;
            a = &a[1..];
        } else {
            x1 = b[0].x1;
            x2 = b[0].x2;
            b = &b[1..];
        }

        let mut a_iter = a.iter();
        let mut b_iter = b.iter();

        let mut a_opt = a_iter.next();
        let mut b_opt = b_iter.next();

        while let (Some(a), Some(b)) = (a_opt, b_opt) {
            if a.x1 < b.x1 {
                merge!(a);
                a_opt = a_iter.next();
            } else {
                merge!(b);
                b_opt = b_iter.next();
            }
        }

        while let Some(a) = a_opt {
            merge!(a);
            a_opt = a_iter.next();
        }

        while let Some(b) = b_opt {
            merge!(b);
            b_opt = b_iter.next();
        }

        push!();
    }

    #[inline(always)]
    fn map_t2_to_t1(_tag: NoTag) -> NoTag {
        NoTag
    }

    #[inline(always)]
    fn map_t3_to_t1(_tag: NoTag) -> NoTag {
        NoTag
    }
}

struct Subtract;

impl Op<NoTag, NoTag, NoTag> for Subtract {
    const APPEND_NON_A: bool = true;
    const APPEND_NON_B: bool = false;

    fn handle_band(new: &mut Container, a: &[RectRaw], b: &[RectRaw], y1: i32, y2: i32) {
        let mut x1;
        let mut x2;

        macro_rules! push {
            ($x2:expr) => {
                new.push(RectRaw {
                    x1,
                    y1,
                    x2: $x2,
                    y2,
                    tag: NoTag,
                });
            };
        }

        let mut a_iter = a.iter();
        let mut b_iter = b.iter();

        macro_rules! pull {
            () => {
                match a_iter.next() {
                    Some(n) => {
                        x1 = n.x1;
                        x2 = n.x2;
                    }
                    _ => return,
                }
            };
        }

        pull!();

        let mut b_opt = b_iter.next();

        while let Some(b) = b_opt {
            if b.x2 <= x1 {
                b_opt = b_iter.next();
            } else if b.x1 >= x2 {
                push!(x2);
                pull!();
            } else {
                if b.x1 > x1 {
                    push!(b.x1);
                }
                if b.x2 < x2 {
                    x1 = b.x2;
                } else {
                    pull!();
                }
            }
        }

        loop {
            push!(x2);
            pull!();
        }
    }

    #[inline(always)]
    fn map_t2_to_t1(_tag: NoTag) -> NoTag {
        NoTag
    }

    #[inline(always)]
    fn map_t3_to_t1(_tag: NoTag) -> NoTag {
        NoTag
    }
}

struct Intersect<T>(PhantomData<T>);

impl<T> Op<T, T, NoTag> for Intersect<T>
where
    T: Tag,
{
    const APPEND_NON_A: bool = false;
    const APPEND_NON_B: bool = false;

    fn handle_band(new: &mut Container<T>, a: &[RectRaw<T>], b: &[RectRaw], y1: i32, y2: i32) {
        let mut x1;
        let mut x2;
        let mut tag;

        macro_rules! push {
            ($x2:expr) => {
                new.push(RectRaw {
                    x1,
                    y1,
                    x2: $x2,
                    y2,
                    tag,
                });
            };
        }

        let mut a_iter = a.iter();
        let mut b_iter = b.iter();

        macro_rules! pull {
            () => {
                match a_iter.next() {
                    Some(n) => {
                        x1 = n.x1;
                        x2 = n.x2;
                        tag = n.tag;
                    }
                    _ => return,
                }
            };
        }

        pull!();

        let mut b_opt = b_iter.next();

        while let Some(b) = b_opt {
            if b.x2 <= x1 {
                b_opt = b_iter.next();
            } else if b.x1 >= x2 {
                pull!();
            } else {
                x1 = x1.max(b.x1);
                if x2 <= b.x2 {
                    push!(x2);
                    pull!();
                } else {
                    push!(b.x2);
                    b_opt = b_iter.next();
                }
            }
        }
    }

    #[inline(always)]
    fn map_t2_to_t1(_tag: T) -> T {
        unreachable!()
    }

    #[inline(always)]
    fn map_t3_to_t1(_tag: NoTag) -> T {
        unreachable!()
    }
}

pub fn rects_to_bands(rects_tmp: &[RectRaw]) -> Container {
    rects_to_bands_(rects_tmp)
}

pub fn rects_to_bands_tagged(rects_tmp: &[RectRaw<u32>]) -> Container<u32> {
    rects_to_bands_(rects_tmp)
}

#[inline(always)]
fn rects_to_bands_<T>(rects_tmp: &[RectRaw<T>]) -> Container<T>
where
    T: Tag,
{
    #[derive(Copy, Clone)]
    struct W<T>(RectRaw<T>)
    where
        T: Tag;
    impl<T> Eq for W<T> where T: Tag {}
    impl<T> PartialEq<Self> for W<T>
    where
        T: Tag,
    {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl<T> PartialOrd<Self> for W<T>
    where
        T: Tag,
    {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<T> Ord for W<T>
    where
        T: Tag,
    {
        fn cmp(&self, other: &Self) -> Ordering {
            self.0
                .y1
                .cmp(&other.0.y1)
                .then_with(|| self.0.x1.cmp(&other.0.x1))
                .then_with(|| self.0.tag.cmp(&other.0.tag))
                .reverse()
        }
    }
    impl<T> Deref for W<T>
    where
        T: Tag,
    {
        type Target = RectRaw<T>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    let ys = {
        let mut tmp: Vec<_> = rects_tmp.iter().flat_map(|r| [r.y1, r.y2]).collect();
        tmp.sort_unstable();
        let mut last = None;
        let mut res = vec![];
        for y in tmp {
            if Some(y) != last {
                last = Some(y);
                res.push(y);
            }
        }
        res
    };

    let mut rects = BinaryHeap::with_capacity(rects_tmp.len());
    for rect in rects_tmp.iter().copied() {
        if !rect.is_empty() {
            rects.push(W(rect));
        }
    }

    let mut res = Container::new();

    for &[y1, y2] in ys.array_windows_ext::<2>() {
        #[expect(clippy::never_loop)]
        loop {
            macro_rules! check_rect {
                ($rect:expr) => {{
                    if $rect.y1 != y1 {
                        break;
                    }
                    rects.pop();
                    if y2 < $rect.y2 {
                        $rect.0.y1 = y2;
                        rects.push($rect);
                    }
                }};
            }
            if let Some(mut rect) = rects.peek().copied() {
                check_rect!(rect);
                let mut x1 = rect.x1;
                let mut x2 = rect.x2;
                let mut tag: T = rect.tag;
                while let Some(mut rect) = rects.peek().copied() {
                    check_rect!(rect);
                    if rect.x1 > x2 || (rect.tag != tag && rect.x1 == x2) {
                        res.push(RectRaw {
                            x1,
                            x2,
                            y1,
                            y2,
                            tag: tag.constrain(),
                        });
                        x1 = rect.x1;
                        x2 = rect.x2;
                        tag = rect.tag;
                    } else {
                        if rect.tag == tag {
                            x2 = x2.max(rect.x2);
                        } else if rect.tag > tag {
                            if rect.x2 > x2 {
                                rect.0.x1 = x2;
                                rect.0.y1 = y1;
                                rect.0.y2 = y2;
                                rects.push(rect);
                            }
                        } else {
                            if x2 > rect.x2 {
                                rects.push(W(RectRaw {
                                    x1: rect.x2,
                                    y1,
                                    x2,
                                    y2,
                                    tag,
                                }));
                            }
                            res.push(RectRaw {
                                x1,
                                y1,
                                x2: rect.x1,
                                y2,
                                tag: tag.constrain(),
                            });
                            x1 = rect.x1;
                            x2 = rect.x2;
                            tag = rect.tag;
                        }
                    }
                }
                res.push(RectRaw {
                    x1,
                    x2,
                    y1,
                    y2,
                    tag: tag.constrain(),
                });
            }
            break;
        }
    }

    let mut needs_merge = false;
    let mut num_elements = res.len();
    let mut bands = Bands { rects: &res }.peekable();
    while let Some(band) = bands.next() {
        let next = match bands.peek() {
            Some(next) => next,
            _ => break,
        };
        if band.can_merge_with(next) {
            needs_merge = true;
            num_elements -= band.rects.len();
        }
    }

    if !needs_merge {
        res.shrink_to_fit();
        return res;
    }

    let mut merged = Container::with_capacity(num_elements);
    let mut bands = Bands { rects: &res }.peekable();
    while let Some(mut band) = bands.next() {
        while let Some(next) = bands.peek() {
            if band.can_merge_with(next) {
                band.y2 = next.y2;
                bands.next();
            } else {
                break;
            }
        }
        for mut rect in band.rects.iter().copied() {
            rect.y2 = band.y2;
            merged.push(rect);
        }
    }

    merged
}

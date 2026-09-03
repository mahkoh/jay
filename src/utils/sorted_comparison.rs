use std::cmp::Ordering;

#[cfg(test)]
mod tests;

pub fn sorted_comparison_by<'a, T>(
    a1: &'a [T],
    a2: &'a [T],
    cmp: impl Fn(&T, &T) -> Ordering,
) -> impl Iterator<Item = SortedResult<'a, T>> {
    SortedComparison {
        a1,
        a2,
        p1: 0,
        p2: 0,
        cmp,
    }
}

#[cfg_attr(not(test), expect(unused))]
pub fn sorted_comparison<'a, T>(
    a1: &'a [T],
    a2: &'a [T],
) -> impl Iterator<Item = SortedResult<'a, T>>
where
    T: Ord,
{
    sorted_comparison_by(a1, a2, |a, b| a.cmp(b))
}

struct SortedComparison<'a, T, Cmp> {
    a1: &'a [T],
    a2: &'a [T],
    p1: usize,
    p2: usize,
    cmp: Cmp,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SortedResult<'a, T> {
    Left(&'a T),
    Right(&'a T),
    Equal(&'a T, &'a T),
}

impl<'a, T, Cmp> Iterator for SortedComparison<'a, T, Cmp>
where
    Cmp: Fn(&T, &T) -> Ordering,
{
    type Item = SortedResult<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.p1 < self.a1.len() {
            let t1 = &self.a1[self.p1];
            if self.p2 < self.a2.len() {
                let t2 = &self.a2[self.p2];
                match (self.cmp)(t1, t2) {
                    Ordering::Less => {
                        self.p1 += 1;
                        Some(SortedResult::Left(t1))
                    }
                    Ordering::Equal => {
                        self.p1 += 1;
                        self.p2 += 1;
                        Some(SortedResult::Equal(t1, t2))
                    }
                    Ordering::Greater => {
                        self.p2 += 1;
                        Some(SortedResult::Right(t2))
                    }
                }
            } else {
                self.p1 += 1;
                Some(SortedResult::Left(t1))
            }
        } else {
            if self.p2 < self.a2.len() {
                let t2 = &self.a2[self.p2];
                self.p2 += 1;
                Some(SortedResult::Right(t2))
            } else {
                None
            }
        }
    }
}

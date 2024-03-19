use std::{
    collections::{Bound, VecDeque},
    ops::RangeBounds,
};

pub trait VecDequeExt<T> {
    fn get_slices(&self, range: impl RangeBounds<usize>) -> (&[T], &[T]);
}

impl<T> VecDequeExt<T> for VecDeque<T> {
    fn get_slices(&self, range: impl RangeBounds<usize>) -> (&[T], &[T]) {
        let (l, r) = self.as_slices();
        let start = match range.start_bound().cloned() {
            Bound::Included(n) => n,
            Bound::Excluded(n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound().cloned() {
            Bound::Included(n) => n + 1,
            Bound::Excluded(n) => n,
            Bound::Unbounded => self.len(),
        };
        let left = {
            let lo = start.min(l.len());
            let hi = end.min(l.len());
            &l[lo..hi]
        };
        let right = {
            let lo = start.saturating_sub(l.len());
            let hi = end.saturating_sub(l.len());
            &r[lo..hi]
        };
        (left, right)
    }
}

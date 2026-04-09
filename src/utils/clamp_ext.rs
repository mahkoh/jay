pub trait ClampExt: Ord {
    fn clamp_saturating(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

impl<T> ClampExt for T where T: Ord {}

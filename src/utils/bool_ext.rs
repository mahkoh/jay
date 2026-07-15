pub trait BoolExt {
    fn and_then<T>(self, f: impl FnOnce() -> Option<T>) -> Option<T>;
}

impl BoolExt for bool {
    fn and_then<T>(self, f: impl FnOnce() -> Option<T>) -> Option<T> {
        match self {
            true => f(),
            false => None,
        }
    }
}

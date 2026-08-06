pub trait KeepAlive {}

impl<T: ?Sized> KeepAlive for T {}

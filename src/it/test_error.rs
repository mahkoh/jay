use {
    crate::utils::errorfmt::ErrorFmt,
    std::{
        error::Error,
        fmt::{Debug, Display, Formatter},
    },
};

pub type TestResult<T = ()> = Result<T, TestError>;

pub struct TestError {
    error: Box<dyn Error + 'static>,
    source: Option<Box<TestError>>,
}

impl TestError {
    pub fn new<D: Display + 'static>(d: D) -> Self {
        Self {
            error: Box::new(DisplayError { msg: d }),
            source: None,
        }
    }
}

struct DisplayError<T: Display> {
    msg: T,
}

impl<T: Display> Debug for DisplayError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.msg, f)
    }
}

impl<T: Display> Display for DisplayError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.msg, f)
    }
}

impl<T: Display> Error for DisplayError<T> {}

impl Debug for TestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestError")
            .field("error", &self.error)
            .field("source", &self.source)
            .finish()
    }
}

impl Display for TestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut e_prev = self;
        let mut e_opt = Some(self);
        let mut first = true;
        while let Some(e) = e_opt {
            if first {
                write!(f, "{}", e.error)?;
                first = false;
            } else {
                write!(f, ": {}", e.error)?;
            }
            e_prev = e;
            e_opt = e.source.as_deref();
        }
        if let Some(e) = e_prev.error.source() {
            write!(f, ": ")?;
            ErrorFmt(e).fmt(f)?;
        }
        Ok(())
    }
}

impl<T: Error + 'static> From<T> for TestError {
    fn from(error: T) -> Self {
        Self {
            error: Box::new(error),
            source: None,
        }
    }
}

pub trait TestErrorExt {
    type Context;

    fn with_context<T, F>(self, f: F) -> Self::Context
    where
        T: Display + 'static,
        F: FnOnce() -> T;
}

impl<T, E> TestErrorExt for Result<T, E>
where
    E: StdError,
{
    type Context = Result<T, TestError>;

    fn with_context<D, F>(self, f: F) -> Self::Context
    where
        D: Display + 'static,
        F: FnOnce() -> D,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(e.with_context(f())),
        }
    }
}

pub trait StdError: 'static {
    fn with_context<D: Display + 'static>(self, d: D) -> TestError;
}

impl<E: Error + 'static> StdError for E {
    fn with_context<D: Display + 'static>(self, d: D) -> TestError {
        TestError {
            error: Box::new(DisplayError { msg: d }),
            source: Some(Box::new(self.into())),
        }
    }
}

impl StdError for TestError {
    fn with_context<D: Display + 'static>(self, d: D) -> TestError {
        TestError {
            error: Box::new(DisplayError { msg: d }),
            source: Some(Box::new(self)),
        }
    }
}

macro_rules! bail {
    ($($tt:tt)*) => {{
        let msg = format!($($tt)*);
        return Err(crate::it::test_error::TestError::new(msg));
    }}
}

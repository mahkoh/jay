use std::{
    error::Error,
    fmt::{Display, Formatter},
};

pub struct ErrorFmt<E>(pub E);

impl<E: Error> Display for ErrorFmt<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut e_opt = Some(&self.0 as &dyn Error);
        let mut first = true;
        while let Some(e) = e_opt {
            if first {
                write!(f, "{}", e)?;
                first = false;
            } else {
                write!(f, ": {}", e)?;
            }
            e_opt = e.source();
        }
        Ok(())
    }
}

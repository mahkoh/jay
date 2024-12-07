use {
    crate::{
        config::error::SpannedError,
        toml::{
            toml_parser::{ErrorHandler, ParserError},
            toml_span::{Span, Spanned},
        },
    },
    ahash::AHashSet,
    error_reporter::Report,
    std::{cell::RefCell, convert::Infallible, error::Error},
};

pub struct Context<'a> {
    pub input: &'a [u8],
    pub used: RefCell<Used>,
}

#[derive(Default)]
pub struct Used {
    pub outputs: Vec<Spanned<String>>,
    pub inputs: Vec<Spanned<String>>,
    pub drm_devices: Vec<Spanned<String>>,
    pub keymaps: Vec<Spanned<String>>,
    pub defined_outputs: AHashSet<Spanned<String>>,
    pub defined_inputs: AHashSet<Spanned<String>>,
    pub defined_drm_devices: AHashSet<Spanned<String>>,
    pub defined_keymaps: AHashSet<Spanned<String>>,
}

impl<'a> Context<'a> {
    pub fn error<E: Error>(&self, cause: Spanned<E>) -> SpannedError<'a, E> {
        self.error2(cause.span, cause.value)
    }

    pub fn error2<E: Error>(&self, span: Span, cause: E) -> SpannedError<'a, E> {
        SpannedError {
            input: self.input.into(),
            span,
            cause: Some(cause),
        }
    }

    pub fn error3(&self, span: Span) -> SpannedError<'a, Infallible> {
        SpannedError {
            input: self.input.into(),
            span,
            cause: None,
        }
    }
}

impl ErrorHandler for Context<'_> {
    fn handle(&self, err: Spanned<ParserError>) {
        log::warn!("{}", Report::new(self.error(err)));
    }

    fn redefinition(&self, err: Spanned<ParserError>, prev: Span) {
        log::warn!("{}", Report::new(self.error(err)));
        log::info!(
            "Previous definition here: {}",
            Report::new(self.error3(prev))
        );
    }
}

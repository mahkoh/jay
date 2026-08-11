use crate::config::WorkspaceSlot;
use crate::config::counter::CounterSlot;
use crate::config::error::SpannedError;
use crate::toml::toml_parser::ErrorHandler;
use crate::toml::toml_parser::ParserError;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use ahash::AHashMap;
use ahash::AHashSet;
use error_reporter::Report;
use std::cell::RefCell;
use std::convert::Infallible;
use std::error::Error;
use std::rc::Rc;

pub struct Context<'a, 'c> {
    pub input: &'a [u8],
    pub used: RefCell<Used>,
    pub mark_names: &'c RefCell<AHashMap<String, u32>>,
    pub workspaces: RefCell<&'c mut AHashMap<String, Rc<WorkspaceSlot>>>,
    pub counters: RefCell<AHashMap<String, Rc<CounterSlot>>>,
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

impl<'a> Context<'a, '_> {
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

impl ErrorHandler for Context<'_, '_> {
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

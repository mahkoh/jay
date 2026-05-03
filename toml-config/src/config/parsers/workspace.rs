use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::{
            toml_span::{Span, Spanned},
            toml_value::Value,
        },
    },
    ahash::AHashMap,
    indexmap::IndexMap,
    jay_config::Workspace,
    std::{cell::Cell, collections::hash_map::Entry, fmt::Debug, rc::Rc},
    thiserror::Error,
};

#[derive(Debug)]
pub struct WorkspaceSlot {
    pub ws: Cell<Workspace>,
    pub ty: Cell<WorkspaceType>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceType {
    Normal,
}

impl Context<'_> {
    pub fn get_workspace_slot(&self, name: &str) -> Rc<WorkspaceSlot> {
        let map = &mut *self.workspaces.borrow_mut();
        if let Some(ws) = map.get(name) {
            return ws.clone();
        }
        let ws = Rc::new(WorkspaceSlot {
            ws: Cell::new(Workspace(0)),
            ty: Cell::new(WorkspaceType::Normal),
        });
        map.insert(name.to_string(), ws.clone());
        ws
    }
}

#[derive(Debug, Error)]
pub enum WorkspaceParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct WorkspacesParser<'a>(pub &'a Context<'a>);

impl Parser for WorkspacesParser<'_> {
    type Value = ();
    type Error = WorkspaceParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut seen_names = AHashMap::default();
        for (name, def) in table {
            match seen_names.entry(name.value.clone()) {
                Entry::Occupied(e) => {
                    log::warn!(
                        "Duplicate workspace definition: {}",
                        self.0.error3(name.span)
                    );
                    log::warn!("Previous definition here: {}", self.0.error3(*e.get()));
                }
                Entry::Vacant(e) => {
                    e.insert(name.span);
                }
            }
            let mut parser = WorkspaceParser {
                name: &name.value,
                cx: self.0,
            };
            if let Err(e) = def.parse(&mut parser) {
                log::error!("Could not parse workspace: {}", self.0.error(e));
            }
        }
        Ok(())
    }
}

pub struct WorkspaceParser<'a> {
    name: &'a str,
    cx: &'a Context<'a>,
}

impl Parser for WorkspaceParser<'_> {
    type Value = ();
    type Error = WorkspaceParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let _ext = Extractor::new(self.cx, span, table);
        let _ws = self.cx.get_workspace_slot(self.name);
        Ok(())
    }
}

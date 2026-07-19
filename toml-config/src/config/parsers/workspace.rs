use crate::State;
use crate::config::OutputMatch;
use crate::config::TomlWorkspace;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::output_match::OutputMatchParser;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use ahash::AHashMap;
use indexmap::IndexMap;
use jay_config::Workspace;
use jay_config::video::Connector;
use jay_config::video::connectors;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug)]
pub struct WorkspaceSlot {
    pub ws: Cell<Workspace>,
    pub implicit_ty: Cell<WorkspaceType>,
    pub explicit_ty: Cell<Option<WorkspaceType>>,
    pub implicit_output: RefCell<Option<Rc<OutputMatch>>>,
    pub explicit_output: RefCell<Option<Rc<OutputMatch>>>,
}

impl WorkspaceSlot {
    pub fn to_toml(&self) -> TomlWorkspace {
        TomlWorkspace {
            ws: self.ws.get(),
            _ty: self.explicit_ty.get().unwrap_or(self.implicit_ty.get()),
            output: self
                .explicit_output
                .borrow()
                .clone()
                .or(self.implicit_output.borrow().clone()),
            output_matched: Default::default(),
        }
    }
}

impl TomlWorkspace {
    pub fn handle_connector_connected(&self, state: &State, c: Connector) {
        if self.output_matched.get().is_some() {
            return;
        }
        if let Some(matcher) = &self.output
            && matcher.matches(c, state)
        {
            self.set_initial_output(state, Some(c));
        }
    }

    pub fn handle_connector_disconnected(&self, state: &State, c: Connector) {
        if self.output_matched.get() != Some(c) {
            return;
        }
        self.determine_initial_output(state);
    }

    pub fn determine_initial_output(&self, state: &State) {
        self.determine_initial_output2(state, &connectors());
    }

    pub fn determine_initial_output2(&self, state: &State, connectors: &[Connector]) {
        if let Some(matcher) = &self.output {
            for &c in connectors {
                if matcher.matches(c, state) {
                    self.set_initial_output(state, Some(c));
                    return;
                }
            }
        }
        self.set_initial_output(state, None);
    }

    fn set_initial_output(&self, state: &State, connector: Option<Connector>) {
        if self.output_matched.get() == connector {
            return;
        }
        self.output_matched.set(connector);
        self.ws.set_initial_connector(connector);
        let wwio = &mut *state
            .persistent
            .workspaces_with_initial_outputs
            .borrow_mut();
        if connector.is_some() {
            wwio.insert(self.ws);
        } else {
            wwio.remove(&self.ws);
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceType {
    Normal,
    Overlay,
}

impl Context<'_> {
    pub fn get_workspace_slot(&self, name: &str) -> Rc<WorkspaceSlot> {
        let map = &mut *self.workspaces.borrow_mut();
        if let Some(ws) = map.get(name) {
            return ws.clone();
        }
        let ws = Rc::new(WorkspaceSlot {
            ws: Cell::new(Workspace(0)),
            implicit_ty: Cell::new(WorkspaceType::Normal),
            explicit_ty: Default::default(),
            implicit_output: Default::default(),
            explicit_output: Default::default(),
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

pub struct WorkspacesParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for WorkspacesParser<'_, '_> {
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

pub struct WorkspaceParser<'a, 'b> {
    name: &'a str,
    cx: &'a Context<'b>,
}

impl Parser for WorkspaceParser<'_, '_> {
    type Value = ();
    type Error = WorkspaceParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (ty_str, initial_output) =
            ext.extract((recover(opt(str("type"))), opt(val("initial-output"))))?;
        let ws = self.cx.get_workspace_slot(self.name);
        if let Some(v) = initial_output {
            match v.parse(&mut OutputMatchParser(self.cx)) {
                Ok(v) => *ws.explicit_output.borrow_mut() = Some(Rc::new(v)),
                Err(e) => {
                    log::error!("Could not parse initial output: {}", self.cx.error(e));
                }
            }
        }
        'ty: {
            if let Some(ty_str) = ty_str {
                let ty = match ty_str.value {
                    "normal" => WorkspaceType::Normal,
                    "overlay" => WorkspaceType::Overlay,
                    _ => {
                        log::error!("Unknown workspace type: {}", self.cx.error3(ty_str.span));
                        break 'ty;
                    }
                };
                ws.explicit_ty.set(Some(ty));
            }
        }
        Ok(())
    }
}

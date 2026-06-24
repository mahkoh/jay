use {
    crate::{
        config::{
            context::Context,
            extractor::{Extractor, ExtractorError, opt, recover, str},
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
    pub implicit_ty: Cell<WorkspaceType>,
    pub explicit_ty: Cell<Option<WorkspaceType>>,
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
        let (ty_str,) = ext.extract((recover(opt(str("type"))),))?;
        let ws = self.cx.get_workspace_slot(self.name);
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

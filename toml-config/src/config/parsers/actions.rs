use {
    crate::{
        config::{
            Action, NamedAction,
            context::Context,
            extractor::ExtractorError,
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::action::ActionParser,
        },
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    std::{collections::HashSet, rc::Rc},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ActionsParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    ExtractorError(#[from] ExtractorError),
}

pub struct ActionsParser<'a, 'b> {
    pub cx: &'a Context<'a>,
    pub used_names: HashSet<Spanned<Rc<String>>>,
    pub actions: &'b mut Vec<NamedAction>,
}

impl Parser for ActionsParser<'_, '_> {
    type Value = ();
    type Error = ActionsParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        _span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        for (name, value) in table.iter() {
            let Some(action) = parse_action(self.cx, &name.value, value) else {
                continue;
            };
            let name = Rc::new(name.value.clone()).spanned(name.span);
            log_used(self.cx, &mut self.used_names, name.clone());
            self.actions.push(NamedAction {
                name: name.value,
                action,
            });
        }
        Ok(())
    }
}

fn parse_action(cx: &Context<'_>, name: &str, value: &Spanned<Value>) -> Option<Action> {
    match value.parse(&mut ActionParser(cx)) {
        Ok(a) => Some(a),
        Err(e) => {
            log::warn!("Could not parse action for name {name}: {}", cx.error(e));
            None
        }
    }
}

fn log_used(cx: &Context<'_>, used: &mut HashSet<Spanned<Rc<String>>>, key: Spanned<Rc<String>>) {
    if let Some(prev) = used.get(&key) {
        log::warn!(
            "Duplicate actions overrides previous definition: {}",
            cx.error3(key.span)
        );
        log::info!("Previous definition here: {}", cx.error3(prev.span));
    }
    used.insert(key);
}

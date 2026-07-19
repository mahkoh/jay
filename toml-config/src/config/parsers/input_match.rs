use crate::config::InputMatch;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::opt;
use crate::config::extractor::str;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InputMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct InputMatchParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for InputMatchParser<'_, '_> {
    type Value = InputMatch;
    type Error = InputMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(self) {
                Ok(m) => res.push(m),
                Err(e) => {
                    log::error!("Could not parse match rule: {}", self.0.error(e));
                }
            }
        }
        Ok(InputMatch::Any(res))
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            (
                tag,
                name,
                syspath,
                devnode,
                is_keyboard,
                is_pointer,
                is_touch,
                is_tablet_tool,
                is_tablet_pad,
                is_gesture,
            ),
            (is_switch,),
        ) = ext.extract((
            (
                opt(str("tag")),
                opt(str("name")),
                opt(str("syspath")),
                opt(str("devnode")),
                opt(bol("is-keyboard")),
                opt(bol("is-pointer")),
                opt(bol("is-touch")),
                opt(bol("is-tablet-tool")),
                opt(bol("is-tablet-pad")),
                opt(bol("is-gesture")),
            ),
            (opt(bol("is-switch")),),
        ))?;
        if let Some(tag) = tag {
            self.0.used.borrow_mut().inputs.push(tag.into());
        }
        Ok(InputMatch::All {
            tag: tag.despan_into(),
            name: name.despan_into(),
            syspath: syspath.despan_into(),
            devnode: devnode.despan_into(),
            is_keyboard: is_keyboard.despan(),
            is_pointer: is_pointer.despan(),
            is_touch: is_touch.despan(),
            is_tablet_tool: is_tablet_tool.despan(),
            is_tablet_pad: is_tablet_pad.despan(),
            is_gesture: is_gesture.despan(),
            is_switch: is_switch.despan(),
        })
    }
}

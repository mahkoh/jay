use {
    crate::{
        config::{
            context::Context,
            extractor::{flt, opt, s32, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            Mode,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ModeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct ModeParser<'a>(pub &'a Context<'a>);

impl<'a> Parser for ModeParser<'a> {
    type Value = Mode;
    type Error = ModeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (width, height, refresh_rate) =
            ext.extract((s32("width"), s32("height"), opt(flt("refresh-rate"))))?;
        Ok(Mode {
            width: width.value,
            height: height.value,
            refresh_rate: refresh_rate.despan(),
        })
    }
}

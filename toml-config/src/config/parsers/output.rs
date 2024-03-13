use {
    crate::{
        config::{
            context::Context,
            extractor::{fltorint, opt, recover, s32, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                mode::ModeParser,
                output_match::{OutputMatchParser, OutputMatchParserError},
            },
            Output,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::video::Transform,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum OutputParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] OutputMatchParserError),
}

pub struct OutputParser<'a> {
    pub cx: &'a Context<'a>,
    pub name_ok: bool,
}

impl<'a> Parser for OutputParser<'a> {
    type Value = Output;
    type Error = OutputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (name, match_val, x, y, scale, transform, mode) = ext.extract((
            opt(str("name")),
            val("match"),
            recover(opt(s32("x"))),
            recover(opt(s32("y"))),
            recover(opt(fltorint("scale"))),
            recover(opt(str("transform"))),
            opt(val("mode")),
        ))?;
        let transform = match transform {
            None => None,
            Some(t) => match t.value {
                "none" => Some(Transform::None),
                "rotate-90" => Some(Transform::Rotate90),
                "rotate-180" => Some(Transform::Rotate180),
                "rotate-270" => Some(Transform::Rotate270),
                "flip" => Some(Transform::Flip),
                "flip-rotate-90" => Some(Transform::FlipRotate90),
                "flip-rotate-180" => Some(Transform::FlipRotate180),
                "flip-rotate-270" => Some(Transform::FlipRotate270),
                _ => {
                    log::warn!("Unknown transform {}: {}", t.value, self.cx.error3(t.span));
                    None
                }
            },
        };
        let mode = match mode {
            Some(mode) => match mode.parse(&mut ModeParser(self.cx)) {
                Ok(m) => Some(m),
                Err(e) => {
                    log::warn!("Could not parse mode: {}", self.cx.error(e));
                    None
                }
            },
            None => None,
        };
        if let Some(name) = name {
            if self.name_ok {
                self.cx
                    .used
                    .borrow_mut()
                    .defined_outputs
                    .insert(name.into());
            } else {
                log::warn!(
                    "Output names have no effect in this position (did you mean match.name?): {}",
                    self.cx.error3(name.span)
                );
            }
        }
        Ok(Output {
            name: name.despan().map(|v| v.to_string()),
            match_: match_val.parse_map(&mut OutputMatchParser(self.cx))?,
            x: x.despan(),
            y: y.despan(),
            scale: scale.despan(),
            transform,
            mode,
        })
    }
}

pub struct OutputsParser<'a>(pub &'a Context<'a>);

impl<'a> Parser for OutputsParser<'a> {
    type Value = Vec<Output>;
    type Error = OutputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(&mut OutputParser {
                cx: self.0,
                name_ok: true,
            }) {
                Ok(o) => res.push(o),
                Err(e) => {
                    log::warn!("Could not parse output: {}", self.0.error(e));
                }
            }
        }
        Ok(res)
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        log::warn!(
            "`outputs` value should be an array: {}",
            self.0.error3(span)
        );
        OutputParser {
            cx: self.0,
            name_ok: true,
        }
        .parse_table(span, table)
        .map(|v| vec![v])
    }
}

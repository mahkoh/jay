use {
    crate::{
        config::{
            Output,
            context::Context,
            extractor::{Extractor, ExtractorError, fltorint, opt, recover, s32, str, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                format::FormatParser,
                mode::ModeParser,
                output_match::{OutputMatchParser, OutputMatchParserError},
                tearing::TearingParser,
                vrr::VrrParser,
            },
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::video::{ColorSpace, TransferFunction, Transform},
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

impl Parser for OutputParser<'_> {
    type Value = Output;
    type Error = OutputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            (name, match_val, x, y, scale, transform, mode, vrr_val, tearing_val, format_val),
            (color_space, transfer_function),
        ) = ext.extract((
            (
                opt(str("name")),
                val("match"),
                recover(opt(s32("x"))),
                recover(opt(s32("y"))),
                recover(opt(fltorint("scale"))),
                recover(opt(str("transform"))),
                opt(val("mode")),
                opt(val("vrr")),
                opt(val("tearing")),
                opt(val("format")),
            ),
            (
                recover(opt(str("color-space"))),
                recover(opt(str("transfer-function"))),
            ),
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
        let color_space = match color_space {
            None => None,
            Some(cs) => match cs.value {
                "default" => Some(ColorSpace::DEFAULT),
                "bt2020" => Some(ColorSpace::BT2020),
                _ => {
                    log::warn!(
                        "Unknown color space {}: {}",
                        cs.value,
                        self.cx.error3(cs.span)
                    );
                    None
                }
            },
        };
        let transfer_function = match transfer_function {
            None => None,
            Some(tf) => match tf.value {
                "default" => Some(TransferFunction::DEFAULT),
                "pq" => Some(TransferFunction::PQ),
                _ => {
                    log::warn!(
                        "Unknown transfer function {}: {}",
                        tf.value,
                        self.cx.error3(tf.span)
                    );
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
        let mut vrr = None;
        if let Some(value) = vrr_val {
            match value.parse(&mut VrrParser(self.cx)) {
                Ok(v) => vrr = Some(v),
                Err(e) => {
                    log::warn!("Could not parse VRR setting: {}", self.cx.error(e));
                }
            }
        }
        let mut tearing = None;
        if let Some(value) = tearing_val {
            match value.parse(&mut TearingParser(self.cx)) {
                Ok(v) => tearing = Some(v),
                Err(e) => {
                    log::warn!("Could not parse tearing setting: {}", self.cx.error(e));
                }
            }
        }
        let mut format = None;
        if let Some(value) = format_val {
            match value.parse(&mut FormatParser) {
                Ok(v) => format = Some(v),
                Err(e) => {
                    log::warn!(
                        "Could not parse framebuffer format setting: {}",
                        self.cx.error(e)
                    );
                }
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
            vrr,
            tearing,
            format,
            color_space,
            transfer_function,
        })
    }
}

pub struct OutputsParser<'a>(pub &'a Context<'a>);

impl Parser for OutputsParser<'_> {
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

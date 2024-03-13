use {
    crate::{
        config::{
            context::Context,
            extractor::{bol, fltorint, opt, recover, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::input_match::{InputMatchParser, InputMatchParserError},
            Input,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    jay_config::input::acceleration::{ACCEL_PROFILE_ADAPTIVE, ACCEL_PROFILE_FLAT},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum InputParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] InputMatchParserError),
    #[error("Transform matrix must have exactly two rows")]
    TwoRows,
    #[error("Transform matrix must have exactly two columns")]
    TwoColumns,
    #[error("Transform matrix entries must be floats")]
    Float,
}

pub struct InputParser<'a> {
    pub cx: &'a Context<'a>,
    pub tag_ok: bool,
}

impl<'a> Parser for InputParser<'a> {
    type Value = Input;
    type Error = InputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            (
                tag,
                match_val,
                accel_profile,
                accel_speed,
                tap_enabled,
                tap_drag_enabled,
                tap_drag_lock_enabled,
                left_handed,
                natural_scrolling,
                px_per_wheel_scroll,
            ),
            (transform_matrix,),
        ) = ext.extract((
            (
                opt(str("tag")),
                val("match"),
                recover(opt(str("accel-profile"))),
                recover(opt(fltorint("accel-speed"))),
                recover(opt(bol("tap-enabled"))),
                recover(opt(bol("tap-drag-enabled"))),
                recover(opt(bol("tap-drag-lock-enabled"))),
                recover(opt(bol("left-handed"))),
                recover(opt(bol("natural-scrolling"))),
                recover(opt(fltorint("px-per-wheel-scroll"))),
            ),
            (recover(opt(val("transform-matrix"))),),
        ))?;
        let accel_profile = match accel_profile {
            None => None,
            Some(p) => match p.value.to_ascii_lowercase().as_str() {
                "flat" => Some(ACCEL_PROFILE_FLAT),
                "adaptive" => Some(ACCEL_PROFILE_ADAPTIVE),
                v => {
                    log::warn!("Unknown accel-profile {v}: {}", self.cx.error3(p.span));
                    None
                }
            },
        };
        let transform_matrix = match transform_matrix {
            None => None,
            Some(matrix) => match matrix.parse(&mut TransformMatrixParser) {
                Ok(v) => Some(v),
                Err(e) => {
                    log::warn!("Could not parse transform matrix: {}", self.cx.error(e));
                    None
                }
            },
        };
        if let Some(tag) = tag {
            if self.tag_ok {
                self.cx.used.borrow_mut().defined_inputs.insert(tag.into());
            } else {
                log::warn!(
                    "Input tags have no effect in this position (did you mean match.tag?): {}",
                    self.cx.error3(tag.span)
                );
            }
        }
        Ok(Input {
            tag: tag.despan_into(),
            match_: match_val.parse_map(&mut InputMatchParser(self.cx))?,
            accel_profile,
            accel_speed: accel_speed.despan(),
            tap_enabled: tap_enabled.despan(),
            tap_drag_enabled: tap_drag_enabled.despan(),
            tap_drag_lock_enabled: tap_drag_lock_enabled.despan(),
            left_handed: left_handed.despan(),
            natural_scrolling: natural_scrolling.despan(),
            px_per_wheel_scroll: px_per_wheel_scroll.despan(),
            transform_matrix,
        })
    }
}

pub struct InputsParser<'a>(pub &'a Context<'a>);

impl<'a> Parser for InputsParser<'a> {
    type Value = Vec<Input>;
    type Error = InputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(&mut InputParser {
                cx: self.0,
                tag_ok: true,
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
        InputParser {
            cx: self.0,
            tag_ok: true,
        }
        .parse_table(span, table)
        .map(|v| vec![v])
    }
}

struct TransformMatrixParser;

impl Parser for TransformMatrixParser {
    type Value = [[f64; 2]; 2];
    type Error = InputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        if array.len() != 2 {
            return Err(InputParserError::TwoRows.spanned(span));
        }
        Ok([
            array[0].parse(&mut TransformMatrixRowParser)?,
            array[1].parse(&mut TransformMatrixRowParser)?,
        ])
    }
}

struct TransformMatrixRowParser;

impl Parser for TransformMatrixRowParser {
    type Value = [f64; 2];
    type Error = InputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        if array.len() != 2 {
            return Err(InputParserError::TwoColumns.spanned(span));
        }
        let extract = |v: &Spanned<Value>| match v.value {
            Value::Float(f) => Ok(f),
            Value::Integer(f) => Ok(f as _),
            _ => Err(InputParserError::Float.spanned(v.span)),
        };
        Ok([extract(&array[0])?, extract(&array[1])?])
    }
}

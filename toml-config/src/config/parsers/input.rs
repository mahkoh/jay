use {
    crate::{
        config::{
            context::Context,
            extractor::{bol, fltorint, opt, recover, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                action::ActionParser,
                input_match::{InputMatchParser, InputMatchParserError},
                keymap::KeymapParser,
                output_match::OutputMatchParser,
            },
            Input,
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    ahash::AHashMap,
    indexmap::IndexMap,
    jay_config::input::{
        acceleration::{ACCEL_PROFILE_ADAPTIVE, ACCEL_PROFILE_FLAT},
        SwitchEvent,
    },
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
    #[error("Calibration matrix must have exactly two rows")]
    CaliTwoRows,
    #[error("Calibration matrix must have exactly three columns")]
    CaliThreeColumns,
    #[error("Calibration matrix entries must be floats")]
    CaliFloat,
}

pub struct InputParser<'a> {
    pub cx: &'a Context<'a>,
    pub is_inputs_array: bool,
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
            (
                transform_matrix,
                keymap,
                on_lid_opened_val,
                on_lid_closed_val,
                on_converted_to_laptop_val,
                on_converted_to_tablet_val,
                output_val,
                remove_mapping,
                calibration_matrix,
            ),
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
            (
                recover(opt(val("transform-matrix"))),
                opt(val("keymap")),
                opt(val("on-lid-opened")),
                opt(val("on-lid-closed")),
                opt(val("on-converted-to-laptop")),
                opt(val("on-converted-to-tablet")),
                opt(val("output")),
                recover(opt(bol("remove-mapping"))),
                recover(opt(val("calibration-matrix"))),
            ),
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
            if self.is_inputs_array {
                self.cx.used.borrow_mut().defined_inputs.insert(tag.into());
            } else {
                log::warn!(
                    "Input tags have no effect in this position (did you mean match.tag?): {}",
                    self.cx.error3(tag.span)
                );
            }
        }
        let keymap = match keymap {
            None => None,
            Some(map) => match map.parse(&mut KeymapParser {
                cx: self.cx,
                definition: false,
            }) {
                Ok(v) => Some(v),
                Err(e) => {
                    log::warn!("Could not parse keymap: {}", self.cx.error(e));
                    None
                }
            },
        };
        let mut switch_actions = AHashMap::new();
        let mut parse_action = |val: Option<Spanned<&Value>>, name, event| {
            if let Some(val) = val {
                if !self.is_inputs_array {
                    log::warn!(
                        "{name} has no effect in this position: {}",
                        self.cx.error3(val.span)
                    );
                    return;
                }
                match val.parse(&mut ActionParser(self.cx)) {
                    Ok(a) => {
                        switch_actions.insert(event, a);
                    }
                    Err(e) => {
                        log::warn!("Could not parse {name} action: {}", self.cx.error(e));
                    }
                }
            }
        };
        parse_action(on_lid_opened_val, "on-lid-opened", SwitchEvent::LidOpened);
        parse_action(on_lid_closed_val, "on-lid-closed", SwitchEvent::LidClosed);
        parse_action(
            on_converted_to_laptop_val,
            "on-converted-to-laptop",
            SwitchEvent::ConvertedToLaptop,
        );
        parse_action(
            on_converted_to_tablet_val,
            "on-converted-to-tablet",
            SwitchEvent::ConvertedToTablet,
        );
        let mut output = None;
        if let Some(val) = output_val {
            match val.parse(&mut OutputMatchParser(self.cx)) {
                Ok(v) => output = Some(Some(v)),
                Err(e) => {
                    log::warn!("Could not parse output: {}", self.cx.error(e));
                }
            }
        }
        if let Some(val) = remove_mapping {
            if self.is_inputs_array {
                log::warn!(
                    "`remove-mapping` has no effect in this position: {}",
                    self.cx.error3(val.span)
                );
            } else if !val.value {
                log::warn!(
                    "`remove-mapping = false` has no effect: {}",
                    self.cx.error3(val.span)
                );
            } else if let Some(output) = output_val {
                log::warn!(
                    "Ignoring `remove-mapping = true` due to conflicting `output` field: {}",
                    self.cx.error3(val.span)
                );
                log::info!(
                    "`output` field defined here: {}",
                    self.cx.error3(output.span)
                );
            } else {
                output = Some(None);
            }
        }
        let calibration_matrix = match calibration_matrix {
            None => None,
            Some(matrix) => match matrix.parse(&mut CalibrationMatrixParser) {
                Ok(v) => Some(v),
                Err(e) => {
                    log::warn!("Could not parse calibration matrix: {}", self.cx.error(e));
                    None
                }
            },
        };
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
            keymap,
            switch_actions,
            output,
            calibration_matrix,
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
                is_inputs_array: true,
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
            is_inputs_array: true,
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

struct CalibrationMatrixParser;

impl Parser for CalibrationMatrixParser {
    type Value = [[f32; 3]; 2];
    type Error = InputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        if array.len() != 2 {
            return Err(InputParserError::CaliTwoRows.spanned(span));
        }
        Ok([
            array[0].parse(&mut CalibrationMatrixRowParser)?,
            array[1].parse(&mut CalibrationMatrixRowParser)?,
        ])
    }
}

struct CalibrationMatrixRowParser;

impl Parser for CalibrationMatrixRowParser {
    type Value = [f32; 3];
    type Error = InputParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array];

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        if array.len() != 3 {
            return Err(InputParserError::CaliThreeColumns.spanned(span));
        }
        let extract = |v: &Spanned<Value>| match v.value {
            Value::Float(f) => Ok(f as f32),
            Value::Integer(f) => Ok(f as _),
            _ => Err(InputParserError::CaliFloat.spanned(v.span)),
        };
        Ok([
            extract(&array[0])?,
            extract(&array[1])?,
            extract(&array[2])?,
        ])
    }
}

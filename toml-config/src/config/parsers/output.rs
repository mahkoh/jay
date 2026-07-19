use crate::config::Output;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::fltorint;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::s32;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::format::FormatParser;
use crate::config::parsers::mode::ModeParser;
use crate::config::parsers::output_match::OutputMatchParser;
use crate::config::parsers::output_match::OutputMatchParserError;
use crate::config::parsers::tearing::TearingParser;
use crate::config::parsers::vrr::VrrParser;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use jay_config::video::BlendSpace;
use jay_config::video::ColorSpace;
use jay_config::video::Eotf;
use jay_config::video::ScalingFilter;
use jay_config::video::Transform;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OutputParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] OutputMatchParserError),
}

pub struct OutputParser<'a, 'b> {
    pub cx: &'a Context<'b>,
    pub name_ok: bool,
}

impl Parser for OutputParser<'_, '_> {
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
            (
                color_space,
                eotf,
                brightness_val,
                blend_space,
                use_native_gamut,
                enabled,
                scaling_filter,
            ),
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
                opt(val("brightness")),
                recover(opt(str("blend-space"))),
                recover(opt(bol("use-native-gamut"))),
                recover(opt(bol("enabled"))),
                recover(opt(str("scaling-filter"))),
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
        let eotf = match eotf {
            None => None,
            Some(tf) => match tf.value {
                "default" => Some(Eotf::DEFAULT),
                "pq" => Some(Eotf::PQ),
                _ => {
                    log::warn!("Unknown EOTF {}: {}", tf.value, self.cx.error3(tf.span));
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
        let mut brightness = None;
        if let Some(value) = brightness_val {
            match value.parse(&mut BrightnessParser) {
                Ok(v) => brightness = Some(v),
                Err(e) => {
                    log::warn!("Could not parse brightness setting: {}", self.cx.error(e));
                }
            }
        }
        let blend_space = match blend_space {
            None => None,
            Some(bs) => match bs.value {
                "linear" => Some(BlendSpace::LINEAR),
                "srgb" => Some(BlendSpace::SRGB),
                _ => {
                    log::warn!(
                        "Unknown blend space {}: {}",
                        bs.value,
                        self.cx.error3(bs.span)
                    );
                    None
                }
            },
        };
        let scaling_filter = match scaling_filter {
            None => None,
            Some(t) => match t.value {
                "linear" => Some(ScalingFilter::LINEAR),
                "nearest" => Some(ScalingFilter::NEAREST),
                _ => {
                    log::warn!(
                        "Unknown scaling filter {}: {}",
                        t.value,
                        self.cx.error3(t.span)
                    );
                    None
                }
            },
        };
        Ok(Output {
            name: name.despan().map(|v| v.to_string()),
            match_: match_val.parse_map(&mut OutputMatchParser(self.cx))?,
            x: x.despan(),
            y: y.despan(),
            scale: scale.despan(),
            scaling_filter,
            transform,
            mode,
            vrr,
            tearing,
            format,
            color_space,
            eotf,
            brightness,
            blend_space,
            use_native_gamut: use_native_gamut.despan(),
            enabled: enabled.despan(),
        })
    }
}

pub struct OutputsParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for OutputsParser<'_, '_> {
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

struct BrightnessParser;

#[derive(Debug, Error)]
pub enum BrightnessParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Expected `default`")]
    UnexpectedString(String),
}

impl Parser for BrightnessParser {
    type Value = Option<f64>;
    type Error = BrightnessParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Float, DataType::Integer, DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        if string == "default" {
            return Ok(None);
        }
        Err(BrightnessParserError::UnexpectedString(string.to_string()).spanned(span))
    }

    fn parse_integer(&mut self, _span: Span, integer: i64) -> ParseResult<Self> {
        Ok(Some(integer as _))
    }

    fn parse_float(&mut self, _span: Span, float: f64) -> ParseResult<Self> {
        Ok(Some(float))
    }
}

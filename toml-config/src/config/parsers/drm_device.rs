use crate::config::ConfigDrmDevice;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::bol;
use crate::config::extractor::fltorint;
use crate::config::extractor::opt;
use crate::config::extractor::recover;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::drm_device_match::DrmDeviceMatchParser;
use crate::config::parsers::drm_device_match::DrmDeviceMatchParserError;
use crate::config::parsers::gfx_api::GfxApiParser;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DrmDeviceParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] DrmDeviceMatchParserError),
}

pub struct DrmDeviceParser<'a, 'b> {
    pub cx: &'a Context<'b>,
    pub name_ok: bool,
}

impl Parser for DrmDeviceParser<'_, '_> {
    type Value = ConfigDrmDevice;
    type Error = DrmDeviceParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (
            name,
            match_val,
            direct_scanout_enabled,
            gfx_api_val,
            flip_margin_ms,
            plane_color_pipelines_enabled,
            flip_margin_auto_adjustment,
        ) = ext.extract((
            opt(str("name")),
            val("match"),
            recover(opt(bol("direct-scanout"))),
            opt(val("gfx-api")),
            recover(opt(fltorint("flip-margin-ms"))),
            recover(opt(bol("plane-color-pipelines"))),
            recover(opt(bol("flip-margin-auto-adjustment"))),
        ))?;
        let gfx_api = match gfx_api_val {
            Some(api) => match api.parse(&mut GfxApiParser) {
                Ok(m) => Some(m),
                Err(e) => {
                    log::warn!("Could not parse graphics API: {}", self.cx.error(e));
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
                    .defined_drm_devices
                    .insert(name.into());
            } else {
                log::warn!(
                    "DRM device names have no effect in this position (did you mean match.name?): {}",
                    self.cx.error3(name.span)
                );
            }
        }
        Ok(ConfigDrmDevice {
            name: name.despan().map(|v| v.to_string()),
            match_: match_val.parse_map(&mut DrmDeviceMatchParser(self.cx))?,
            direct_scanout_enabled: direct_scanout_enabled.despan(),
            gfx_api,
            flip_margin_ms: flip_margin_ms.despan(),
            plane_color_pipelines_enabled: plane_color_pipelines_enabled.despan(),
            flip_margin_auto_adjustment: flip_margin_auto_adjustment.despan(),
        })
    }
}

pub struct DrmDevicesParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for DrmDevicesParser<'_, '_> {
    type Value = Vec<ConfigDrmDevice>;
    type Error = DrmDeviceParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Array];

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut res = vec![];
        for el in array {
            match el.parse(&mut DrmDeviceParser {
                cx: self.0,
                name_ok: true,
            }) {
                Ok(o) => res.push(o),
                Err(e) => {
                    log::warn!("Could not parse drm device: {}", self.0.error(e));
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
            "`drm-devices` value should be an array: {}",
            self.0.error3(span)
        );
        DrmDeviceParser {
            cx: self.0,
            name_ok: true,
        }
        .parse_table(span, table)
        .map(|v| vec![v])
    }
}

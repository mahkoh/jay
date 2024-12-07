use {
    crate::{
        config::{
            context::Context,
            extractor::{bol, fltorint, opt, recover, str, val, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                drm_device_match::{DrmDeviceMatchParser, DrmDeviceMatchParserError},
                gfx_api::GfxApiParser,
            },
            ConfigDrmDevice,
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
pub enum DrmDeviceParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    Match(#[from] DrmDeviceMatchParserError),
}

pub struct DrmDeviceParser<'a> {
    pub cx: &'a Context<'a>,
    pub name_ok: bool,
}

impl Parser for DrmDeviceParser<'_> {
    type Value = ConfigDrmDevice;
    type Error = DrmDeviceParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.cx, span, table);
        let (name, match_val, direct_scanout_enabled, gfx_api_val, flip_margin_ms) =
            ext.extract((
                opt(str("name")),
                val("match"),
                recover(opt(bol("direct-scanout"))),
                opt(val("gfx-api")),
                recover(opt(fltorint("flip-margin-ms"))),
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
        })
    }
}

pub struct DrmDevicesParser<'a>(pub &'a Context<'a>);

impl Parser for DrmDevicesParser<'_> {
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

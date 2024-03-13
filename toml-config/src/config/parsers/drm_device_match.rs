use {
    crate::{
        config::{
            context::Context,
            extractor::{n32, opt, recover, str, Extractor, ExtractorError},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            DrmDeviceMatch,
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
pub enum DrmDeviceMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
}

pub struct DrmDeviceMatchParser<'a>(pub &'a Context<'a>);

impl Parser for DrmDeviceMatchParser<'_> {
    type Value = DrmDeviceMatch;
    type Error = DrmDeviceMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table, DataType::Table];

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
        Ok(DrmDeviceMatch::Any(res))
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (name, syspath, vendor, vendor_name, model, model_name, devnode) = ext.extract((
            recover(opt(str("name"))),
            recover(opt(str("syspath"))),
            recover(opt(n32("pci-vendor"))),
            recover(opt(str("vendor"))),
            recover(opt(n32("pci-model"))),
            recover(opt(str("model"))),
            recover(opt(str("devnode"))),
        ))?;
        if let Some(name) = name {
            self.0.used.borrow_mut().drm_devices.push(name.into());
        }
        Ok(DrmDeviceMatch::All {
            name: name.despan_into(),
            syspath: syspath.despan_into(),
            vendor: vendor.despan(),
            vendor_name: vendor_name.despan_into(),
            model: model.despan(),
            model_name: model_name.despan_into(),
            devnode: devnode.despan_into(),
        })
    }
}

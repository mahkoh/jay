use crate::config::Egui;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::arr;
use crate::config::extractor::opt;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

pub struct EguiParser<'a, 'b>(pub &'a Context<'b>);

#[derive(Debug, Error)]
pub enum EguiParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
}

impl Parser for EguiParser<'_, '_> {
    type Value = Egui;
    type Error = EguiParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (proportional_fonts_arr, monospace_fonts_arr) =
            ext.extract((opt(arr("proportional-fonts")), opt(arr("monospace-fonts"))))?;
        let mut proportional_fonts = None;
        let mut monospace_fonts = None;
        for (out, f) in [
            (&mut proportional_fonts, proportional_fonts_arr),
            (&mut monospace_fonts, monospace_fonts_arr),
        ] {
            if let Some(f) = f {
                let fonts = out.insert(vec![]);
                for f in f.value {
                    let Value::String(s) = &f.value else {
                        log::error!("Expected a string: {}", self.0.error3(f.span));
                        continue;
                    };
                    fonts.push(s.clone());
                }
            }
        }
        Ok(Egui {
            proportional_fonts,
            monospace_fonts,
        })
    }
}

use {
    crate::{
        config::{
            Egui,
            context::Context,
            extractor::{Extractor, ExtractorError, arr, opt},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        },
        toml::{
            toml_span::{Span, Spanned},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

pub struct EguiParser<'a>(pub &'a Context<'a>);

#[derive(Debug, Error)]
pub enum EguiParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
}

impl Parser for EguiParser<'_> {
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

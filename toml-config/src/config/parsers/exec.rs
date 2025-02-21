use {
    crate::{
        config::{
            Exec,
            context::Context,
            extractor::{Extractor, ExtractorError, arr, bol, opt, recover, str, val},
            parser::{DataType, ParseResult, Parser, UnexpectedDataType},
            parsers::{
                StringParser, StringParserError,
                env::{EnvParser, EnvParserError},
            },
        },
        toml::{
            toml_span::{DespanExt, Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    indexmap::IndexMap,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ExecParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extractor(#[from] ExtractorError),
    #[error(transparent)]
    String(#[from] StringParserError),
    #[error(transparent)]
    Env(#[from] EnvParserError),
    #[error("Array cannot be empty")]
    Empty,
}

pub struct ExecParser<'a>(pub &'a Context<'a>);

impl Parser for ExecParser<'_> {
    type Value = Exec;
    type Error = ExecParserError;
    const EXPECTED: &'static [DataType] = &[DataType::String, DataType::Array, DataType::Table];

    fn parse_string(&mut self, _span: Span, string: &str) -> ParseResult<Self> {
        Ok(Exec {
            prog: string.to_string(),
            args: vec![],
            envs: vec![],
            privileged: false,
        })
    }

    fn parse_array(&mut self, span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        if array.is_empty() {
            return Err(ExecParserError::Empty.spanned(span));
        }
        let prog = array[0].parse_map(&mut StringParser)?;
        let mut args = vec![];
        for v in &array[1..] {
            args.push(v.parse_map(&mut StringParser)?);
        }
        Ok(Exec {
            prog,
            args,
            envs: vec![],
            privileged: false,
        })
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (prog, args_val, envs_val, privileged) = ext.extract((
            str("prog"),
            opt(arr("args")),
            opt(val("env")),
            recover(opt(bol("privileged"))),
        ))?;
        let mut args = vec![];
        if let Some(args_val) = args_val {
            for arg in args_val.value {
                args.push(arg.parse_map(&mut StringParser)?);
            }
        }
        let envs = match envs_val {
            None => vec![],
            Some(e) => e.parse_map(&mut EnvParser)?,
        };
        Ok(Exec {
            prog: prog.value.to_string(),
            args,
            envs,
            privileged: privileged.despan().unwrap_or(false),
        })
    }
}

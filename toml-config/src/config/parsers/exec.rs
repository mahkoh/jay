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
    std::sync::LazyLock,
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
    #[error("Exactly one of the `prog` or `shell` fields must be specified")]
    ProgXorShell,
    #[error("Could not read $SHELL")]
    ShellNotDefined,
    #[error("The `args` field cannot be used for shell commands")]
    ArgsForShell,
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
            tag: None,
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
            tag: None,
        })
    }

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (prog_opt, shell_opt, args_val, envs_val, privileged, tag) = ext.extract((
            opt(str("prog")),
            opt(str("shell")),
            opt(arr("args")),
            opt(val("env")),
            recover(opt(bol("privileged"))),
            opt(str("tag")),
        ))?;
        let prog;
        let mut args = vec![];
        match (prog_opt, shell_opt) {
            (None, None) | (Some(_), Some(_)) => {
                return Err(ExecParserError::ProgXorShell.spanned(span));
            }
            (Some(v), _) => {
                prog = v.value.to_string();
                if let Some(args_val) = args_val {
                    for arg in args_val.value {
                        args.push(arg.parse_map(&mut StringParser)?);
                    }
                }
            }
            (_, Some(v)) => {
                prog = shell(v.span)?;
                args = vec!["-c".to_string(), v.value.to_string()];
                if let Some(v) = args_val {
                    return Err(ExecParserError::ArgsForShell.spanned(v.span));
                }
            }
        }
        let envs = match envs_val {
            None => vec![],
            Some(e) => e.parse_map(&mut EnvParser)?,
        };
        if let Some(privileged) = privileged
            && privileged.value
            && tag.is_some()
        {
            log::warn!(
                "Exec is privileged and tagged but tagged execs are always unprivileged: {}",
                self.0.error3(privileged.span),
            );
        }
        Ok(Exec {
            prog,
            args,
            envs,
            privileged: privileged.despan().unwrap_or(false),
            tag: tag.despan_into(),
        })
    }
}

fn shell(span: Span) -> Result<String, Spanned<ExecParserError>> {
    static SHELL: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("SHELL").ok());
    SHELL
        .clone()
        .ok_or(ExecParserError::ShellNotDefined.spanned(span))
}

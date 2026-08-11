use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::opt;
use crate::config::extractor::str;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_span::SpannedExt;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use jay_config::workspace::TileDirection;
use jay_config::workspace::WorkspaceLayout;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceLayoutParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error("Unknown layout type {0}")]
    UnknownType(String),
    #[error("Unknown tile direction {0}")]
    UnknownDirection(String),
}

pub struct WorkspaceLayoutParser<'a, 'b, 'c>(pub &'a Context<'b, 'c>);

impl Parser for WorkspaceLayoutParser<'_, '_, '_> {
    type Value = WorkspaceLayout;
    type Error = WorkspaceLayoutParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let ty = ext.extract_or_ignore(str("type"))?;
        let layout = match ty.value {
            "mono" => WorkspaceLayout::Mono,
            "tile" => {
                let direction = ext.extract(opt(str("direction")))?;
                let direction = match direction {
                    None => TileDirection::Horizontal,
                    Some(d) => match d.value {
                        "horizontal" => TileDirection::Horizontal,
                        "vertical" => TileDirection::Vertical,
                        "major" => TileDirection::Major,
                        "minor" => TileDirection::Minor,
                        _ => {
                            return Err(WorkspaceLayoutParserError::UnknownDirection(
                                d.value.to_owned(),
                            )
                            .spanned(d.span));
                        }
                    },
                };
                WorkspaceLayout::Tile { direction }
            }
            _ => {
                return Err(
                    WorkspaceLayoutParserError::UnknownType(ty.value.to_owned()).spanned(ty.span)
                );
            }
        };
        Ok(layout)
    }
}

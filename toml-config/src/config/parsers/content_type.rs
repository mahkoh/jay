use {
    crate::{
        config::parser::{DataType, ParseResult, Parser, UnexpectedDataType},
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    jay_config::window::{
        ContentType, GAME_CONTENT, NO_CONTENT_TYPE, PHOTO_CONTENT, VIDEO_CONTENT,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ContentTypeParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error("Unknown content type `{}`", .0)]
    UnknownContentType(String),
}

pub struct ContentTypeParser;

impl Parser for ContentTypeParser {
    type Value = ContentType;
    type Error = ContentTypeParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Array, DataType::String];

    fn parse_string(&mut self, span: Span, string: &str) -> ParseResult<Self> {
        let ty = match string {
            "none" => NO_CONTENT_TYPE,
            "any" => !NO_CONTENT_TYPE,
            "photo" => PHOTO_CONTENT,
            "video" => VIDEO_CONTENT,
            "game" => GAME_CONTENT,
            _ => {
                return Err(
                    ContentTypeParserError::UnknownContentType(string.to_owned()).spanned(span),
                );
            }
        };
        Ok(ty)
    }

    fn parse_array(&mut self, _span: Span, array: &[Spanned<Value>]) -> ParseResult<Self> {
        let mut ty = ContentType(0);
        for el in array {
            ty |= el.parse(&mut ContentTypeParser)?;
        }
        Ok(ty)
    }
}

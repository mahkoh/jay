use crate::config::GenericMatch;
use crate::config::MatchExactly;
use crate::config::WindowMatch;
use crate::config::context::Context;
use crate::config::extractor::Extractor;
use crate::config::extractor::ExtractorError;
use crate::config::extractor::arr;
use crate::config::extractor::bol;
use crate::config::extractor::n32;
use crate::config::extractor::opt;
use crate::config::extractor::str;
use crate::config::extractor::val;
use crate::config::parser::DataType;
use crate::config::parser::ParseResult;
use crate::config::parser::Parser;
use crate::config::parser::UnexpectedDataType;
use crate::config::parsers::client_match::ClientMatchParser;
use crate::config::parsers::client_match::ClientMatchParserError;
use crate::config::parsers::content_type::ContentTypeParser;
use crate::config::parsers::content_type::ContentTypeParserError;
use crate::config::parsers::window_type::WindowTypeParser;
use crate::config::parsers::window_type::WindowTypeParserError;
use crate::toml::toml_span::DespanExt;
use crate::toml::toml_span::Span;
use crate::toml::toml_span::Spanned;
use crate::toml::toml_value::Value;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowMatchParserError {
    #[error(transparent)]
    Expected(#[from] UnexpectedDataType),
    #[error(transparent)]
    Extract(#[from] ExtractorError),
    #[error(transparent)]
    WindowTypes(#[from] WindowTypeParserError),
    #[error(transparent)]
    ClientMatchParserError(#[from] ClientMatchParserError),
    #[error(transparent)]
    ContentTypes(#[from] ContentTypeParserError),
}

pub struct WindowMatchParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for WindowMatchParser<'_, '_> {
    type Value = WindowMatch;
    type Error = WindowMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (
            (
                name,
                not_val,
                all_val,
                any_val,
                exactly_val,
                types_val,
                client_val,
                title,
                title_regex,
            ),
            (
                app_id,
                app_id_regex,
                floating,
                visible,
                urgent,
                focused,
                fullscreen,
                just_mapped,
                tag,
                tag_regex,
            ),
            (
                x_class,
                x_class_regex,
                x_instance,
                x_instance_regex,
                x_role,
                x_role_regex,
                workspace,
                workspace_regex,
                content_types_val,
            ),
        ) = ext.extract((
            (
                opt(str("name")),
                opt(val("not")),
                opt(arr("all")),
                opt(arr("any")),
                opt(val("exactly")),
                opt(val("types")),
                opt(val("client")),
                opt(str("title")),
                opt(str("title-regex")),
            ),
            (
                opt(str("app-id")),
                opt(str("app-id-regex")),
                opt(bol("floating")),
                opt(bol("visible")),
                opt(bol("urgent")),
                opt(bol("focused")),
                opt(bol("fullscreen")),
                opt(bol("just-mapped")),
                opt(str("tag")),
                opt(str("tag-regex")),
            ),
            (
                opt(str("x-class")),
                opt(str("x-class-regex")),
                opt(str("x-instance")),
                opt(str("x-instance-regex")),
                opt(str("x-role")),
                opt(str("x-role-regex")),
                opt(str("workspace")),
                opt(str("workspace-regex")),
                opt(val("content-types")),
            ),
        ))?;
        let mut not = None;
        if let Some(value) = not_val {
            not = Some(Box::new(value.parse(&mut WindowMatchParser(self.0))?));
        }
        macro_rules! list {
            ($val:expr) => {{
                let mut list = None;
                if let Some(value) = $val {
                    let mut res = vec![];
                    for value in value.value {
                        res.push(value.parse(&mut WindowMatchParser(self.0))?);
                    }
                    list = Some(res);
                }
                list
            }};
        }
        let all = list!(all_val);
        let any = list!(any_val);
        let mut types = None;
        if let Some(value) = types_val {
            types = Some(value.parse_map(&mut WindowTypeParser)?);
        }
        let mut exactly = None;
        if let Some(value) = exactly_val {
            exactly = Some(value.parse(&mut WindowMatchExactlyParser(self.0))?);
        }
        let mut client = None;
        if let Some(value) = client_val {
            client = Some(value.parse_map(&mut ClientMatchParser(self.0))?);
        }
        let mut content_types = None;
        if let Some(value) = content_types_val {
            content_types = Some(value.parse_map(&mut ContentTypeParser)?);
        }
        Ok(WindowMatch {
            generic: GenericMatch {
                name: name.despan_into(),
                not,
                all,
                any,
                exactly,
            },
            title: title.despan_into(),
            title_regex: title_regex.despan_into(),
            app_id: app_id.despan_into(),
            app_id_regex: app_id_regex.despan_into(),
            floating: floating.despan(),
            visible: visible.despan(),
            urgent: urgent.despan(),
            focused: focused.despan(),
            fullscreen: fullscreen.despan(),
            just_mapped: just_mapped.despan(),
            tag: tag.despan_into(),
            tag_regex: tag_regex.despan_into(),
            x_class: x_class.despan_into(),
            x_class_regex: x_class_regex.despan_into(),
            x_instance: x_instance.despan_into(),
            x_instance_regex: x_instance_regex.despan_into(),
            x_role: x_role.despan_into(),
            x_role_regex: x_role_regex.despan_into(),
            workspace: workspace.despan_into(),
            workspace_regex: workspace_regex.despan_into(),
            types,
            client,
            content_types,
        })
    }
}

pub struct WindowMatchExactlyParser<'a, 'b>(pub &'a Context<'b>);

impl Parser for WindowMatchExactlyParser<'_, '_> {
    type Value = MatchExactly<WindowMatch>;
    type Error = WindowMatchParserError;
    const EXPECTED: &'static [DataType] = &[DataType::Table];

    fn parse_table(
        &mut self,
        span: Span,
        table: &IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> ParseResult<Self> {
        let mut ext = Extractor::new(self.0, span, table);
        let (num, list_val) = ext.extract((n32("num"), arr("list")))?;
        let mut list = vec![];
        for el in list_val.value {
            list.push(el.parse(&mut WindowMatchParser(self.0))?);
        }
        Ok(MatchExactly {
            num: num.value as _,
            list,
        })
    }
}

use {
    crate::toml::{
        toml_lexer::{Lexer, Token},
        toml_span::{Span, Spanned, SpannedExt},
        toml_value::Value,
    },
    bstr::ByteSlice,
    indexmap::{
        map::{raw_entry_v1::RawEntryMut, RawEntryApiV1},
        IndexMap,
    },
    std::{collections::VecDeque, mem, str::FromStr},
    thiserror::Error,
};

pub trait ErrorHandler {
    fn handle(&self, err: Spanned<ParserError>);
    fn redefinition(&self, err: Spanned<ParserError>, prev: Span);
}

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("Unexpected end of file")]
    UnexpectedEof,
    #[error("Expected a key")]
    MissingKey,
    #[error("Expected {0} but found {1}")]
    Expected(&'static str, &'static str),
    #[error("Duplicate key overwrites the previous definition")]
    Redefined,
    #[error("Literal is not valid UTF-8")]
    NonUtf8Literal,
    #[error("Could not parse the literal")]
    UnknownLiteral,
    #[error("Ignoring key due to following error")]
    IgnoringKey,
    #[error("Unnecessary comma")]
    UnnecessaryComma,
}

pub fn parse(
    input: &[u8],
    error_handler: &dyn ErrorHandler,
) -> Result<Spanned<Value>, Spanned<ParserError>> {
    let parser = Parser {
        lexer: Lexer::new(input),
        error_handler,
        last_span: None,
    };
    parser.parse()
}

struct Parser<'a, 'b> {
    lexer: Lexer<'a>,
    error_handler: &'b dyn ErrorHandler,
    last_span: Option<Span>,
}

type Key = VecDeque<Spanned<String>>;

impl<'a> Parser<'a, '_> {
    fn parse(mut self) -> Result<Spanned<Value>, Spanned<ParserError>> {
        self.parse_document()
    }

    fn unexpected_eof(&self) -> Spanned<ParserError> {
        let span = self.last_span.unwrap_or(Span { lo: 0, hi: 0 });
        ParserError::UnexpectedEof.spanned(span)
    }

    fn next(&mut self, value_context: bool) -> Result<Spanned<Token<'a>>, Spanned<ParserError>> {
        match self.lexer.next(value_context) {
            Some(t) => {
                self.last_span = Some(t.span);
                Ok(t)
            }
            _ => Err(self.unexpected_eof()),
        }
    }

    fn peek(&mut self, value_context: bool) -> Result<Spanned<Token<'a>>, Spanned<ParserError>> {
        match self.lexer.peek(value_context) {
            Some(t) => Ok(t),
            _ => Err(self.unexpected_eof()),
        }
    }

    fn parse_value(&mut self) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let token = self.peek(true)?;
        match token.value {
            Token::LiteralString(s) => self.parse_literal_string(s),
            Token::CookedString(s) => self.parse_cooked_string(s),
            Token::LeftBracket => self.parse_array(),
            Token::Literal(l) => self.parse_literal_value(l),
            Token::LeftBrace => self.parse_inline_table(),
            Token::Dot | Token::Equals | Token::Comma | Token::RightBrace | Token::RightBracket => {
                Err(ParserError::Expected("a value", token.value.name(true)).spanned(token.span))
            }
        }
    }

    fn parse_literal_value(
        &mut self,
        literal: &[u8],
    ) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let span = self.next(true)?.span;
        let Ok(s) = std::str::from_utf8(literal) else {
            return Err(ParserError::NonUtf8Literal.spanned(span));
        };
        if s == "true" {
            return Ok(Value::Boolean(true).spanned(span));
        }
        if s == "false" {
            return Ok(Value::Boolean(false).spanned(span));
        }
        let s = s.replace('_', "");
        if let Ok(n) = i64::from_str(&s) {
            return Ok(Value::Integer(n).spanned(span));
        }
        'radix: {
            let b = s.as_bytes();
            if b.len() >= 2 && b[0] == b'0' {
                let radix = match b[1] {
                    b'x' => 16,
                    b'o' => 8,
                    b'b' => 2,
                    _ => break 'radix,
                };
                if let Ok(n) = i64::from_str_radix(&s[2..], radix) {
                    return Ok(Value::Integer(n).spanned(span));
                }
            }
        }
        if let Ok(n) = f64::from_str(&s) {
            return Ok(Value::Float(n).spanned(span));
        }
        Err(ParserError::UnknownLiteral.spanned(span))
    }

    fn parse_literal_string(&mut self, s: &[u8]) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let span = self.next(true)?.span;
        let s = s.as_bstr().to_string();
        Ok(Value::String(s).spanned(span))
    }

    fn parse_cooked_string(&mut self, s: &[u8]) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let span = self.next(true)?.span;
        let s = self.cook_string(s);
        Ok(Value::String(s).spanned(span))
    }

    fn cook_string(&self, s: &[u8]) -> String {
        use std::io::Write;

        if !s.contains(&b'\\') {
            return s.as_bstr().to_string();
        }
        let mut res = vec![];
        let mut pos = 0;
        while pos < s.len() {
            let c = s[pos];
            pos += 1;
            match c {
                b'\\' => {
                    let c = s[pos];
                    pos += 1;
                    match c {
                        b'\\' => res.push(b'\\'),
                        b'"' => res.push(b'"'),
                        b'b' => res.push(0x8),
                        b'f' => res.push(0xc),
                        b'n' => res.push(b'\n'),
                        b'r' => res.push(b'\r'),
                        b't' => res.push(b'\t'),
                        b'e' => res.push(0x1b),
                        b'x' | b'u' | b'U' => 'unicode: {
                            let len = match c {
                                b'x' => 2,
                                b'u' => 4,
                                _ => 8,
                            };
                            if s.len() - pos >= len {
                                if let Ok(s) = std::str::from_utf8(&s[pos..pos + len]) {
                                    if let Ok(n) = u32::from_str_radix(s, 16) {
                                        if let Some(c) = char::from_u32(n) {
                                            pos += len;
                                            let _ = write!(res, "{}", c);
                                            break 'unicode;
                                        }
                                    }
                                }
                            }
                            res.extend_from_slice(&s[pos - 2..]);
                        }
                        b' ' | b'\t' | b'\n' => {
                            let mut t = pos;
                            let mut saw_nl = c == b'\n';
                            while t < s.len() && matches!(s[t], b' ' | b'\t' | b'\n') {
                                saw_nl |= s[t] == b'\n';
                                t += 1;
                            }
                            if saw_nl {
                                pos = t;
                            } else {
                                res.extend_from_slice(&[b'\\', c]);
                            }
                        }
                        _ => {
                            res.extend_from_slice(&[b'\\', c]);
                        }
                    }
                }
                _ => res.push(c),
            }
        }
        res.as_bstr().to_string()
    }

    fn parse_array(&mut self) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let lo = self.next(true)?.span.lo;
        let mut entries = vec![];
        let mut consumed_comma = false;
        loop {
            if let Some(v) = self.lexer.peek(true) {
                if v.value == Token::RightBracket {
                    let _ = self.next(true);
                    let hi = v.span.hi;
                    let span = Span { lo, hi };
                    return Ok(Value::Array(entries).spanned(span));
                }
                if entries.len() > 0 && !mem::take(&mut consumed_comma) {
                    self.error_handler.handle(
                        ParserError::Expected("`,` or `]`", v.value.name(true)).spanned(v.span),
                    );
                }
            }
            match self.parse_value() {
                Ok(v) => {
                    entries.push(v);
                    consumed_comma = self.skip_comma(true);
                }
                Err(e) => {
                    self.skip_tree(Token::LeftBracket, Token::RightBracket);
                    return Err(e);
                }
            }
        }
    }

    fn parse_inline_table(&mut self) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let lo = self.next(true)?.span.lo;
        let mut map = IndexMap::new();
        let mut consumed_comma = false;
        loop {
            let token = match self.peek(false) {
                Ok(t) => t,
                Err(e) => {
                    self.error_handler.handle(e);
                    break;
                }
            };
            if token.value == Token::RightBrace {
                let _ = self.next(false);
                break;
            }
            if !map.is_empty() && !mem::take(&mut consumed_comma) {
                self.error_handler.handle(
                    ParserError::Expected("`,` or `}`", token.value.name(false))
                        .spanned(token.span),
                );
            }
            let res = match self.parse_key_value_with_recovery() {
                Ok(res) => res,
                Err(e) => {
                    self.skip_tree(Token::LeftBrace, Token::RightBrace);
                    return Err(e);
                }
            };
            if let Some((mut key, value)) = res {
                self.insert(&mut map, &mut key, value, false, false);
            };
            consumed_comma = self.skip_comma(false);
        }
        let hi = self.last_span().hi;
        let span = Span { lo, hi };
        Ok(Value::Table(map).spanned(span))
    }

    fn skip_comma(&mut self, value_context: bool) -> bool {
        if let Some(token) = self.lexer.peek(value_context) {
            if token.value != Token::Comma {
                return false;
            }
            let _ = self.next(value_context);
        }
        while let Some(token) = self.lexer.peek(value_context) {
            if token.value != Token::Comma {
                break;
            }
            let _ = self.next(value_context);
            self.error_handler
                .handle(ParserError::UnnecessaryComma.spanned(token.span));
        }
        true
    }

    fn parse_document(&mut self) -> Result<Spanned<Value>, Spanned<ParserError>> {
        let mut map = IndexMap::new();
        self.parse_table_body(&mut map)?;
        while self.lexer.peek(false).is_some() {
            let (mut key, append) = self.parse_table_header()?;
            let mut inner_map = IndexMap::new();
            self.parse_table_body(&mut inner_map)?;
            let value = Value::Table(inner_map).spanned(key.span);
            self.insert(&mut map, &mut key.value, value, true, append);
        }
        let hi = self.last_span().hi;
        let span = Span { lo: 0, hi };
        Ok(Value::Table(map).spanned(span))
    }

    fn parse_table_header(&mut self) -> Result<(Spanned<Key>, bool), Spanned<ParserError>> {
        let lo = self.next(false)?.span.lo;
        let mut append = false;
        if let Some(token) = self.lexer.peek(false) {
            if token.value == Token::LeftBracket {
                let _ = self.next(false);
                append = true;
            }
        }
        let key = self.parse_key()?;
        let mut hi = self.parse_exact(Token::RightBracket, false)?.hi;
        if append {
            hi = self.parse_exact(Token::RightBracket, false)?.hi;
        }
        let span = Span { lo, hi };
        Ok((key.spanned(span), append))
    }

    fn parse_table_body(
        &mut self,
        dst: &mut IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> Result<(), Spanned<ParserError>> {
        while let Some(e) = self.lexer.peek(false) {
            if e.value == Token::LeftBracket {
                return Ok(());
            }
            let Some((mut key, value)) = self.parse_key_value_with_recovery()? else {
                continue;
            };
            self.insert(dst, &mut key, value, false, false);
        }
        Ok(())
    }

    fn insert(
        &self,
        dst: &mut IndexMap<Spanned<String>, Spanned<Value>>,
        keys: &mut Key,
        value: Spanned<Value>,
        modify_array_element: bool,
        append_last: bool,
    ) {
        let key = keys.pop_front().unwrap();
        if keys.is_empty() {
            if let RawEntryMut::Occupied(mut old) =
                dst.raw_entry_mut_v1().from_key(key.value.as_str())
            {
                if append_last {
                    if let Value::Array(array) = &mut old.get_mut().value {
                        array.push(value);
                        return;
                    }
                }
                if let Value::Table(old) = &mut old.get_mut().value {
                    if let Value::Table(new) = value.value {
                        for (k, v) in new {
                            let mut keys = Key::new();
                            keys.push_back(k);
                            self.insert(old, &mut keys, v, false, false);
                        }
                        return;
                    }
                }
                self.error_handler
                    .redefinition(ParserError::Redefined.spanned(key.span), old.key().span);
                old.shift_remove();
            }
            let span = value.span;
            let value = match append_last {
                true => Value::Array(vec![value]).spanned(span),
                false => value,
            };
            dst.insert(key, value);
        } else {
            if let RawEntryMut::Occupied(mut o) = dst.raw_entry_mut_v1().from_key(&key) {
                match &mut o.get_mut().value {
                    Value::Table(dst) => {
                        self.insert(dst, keys, value, modify_array_element, append_last);
                        return;
                    }
                    Value::Array(array) if modify_array_element => {
                        if let Some(Value::Table(dst)) =
                            array.last_mut().as_mut().map(|v| &mut v.value)
                        {
                            self.insert(dst, keys, value, modify_array_element, append_last);
                            return;
                        }
                    }
                    _ => {}
                }
                self.error_handler
                    .redefinition(ParserError::Redefined.spanned(key.span), o.key().span);
                o.shift_remove();
            }
            let mut map = IndexMap::new();
            let span = value.span;
            self.insert(&mut map, keys, value, modify_array_element, append_last);
            dst.insert(key, Value::Table(map).spanned(span));
        }
    }

    fn parse_key_value_with_recovery(
        &mut self,
    ) -> Result<Option<(Key, Spanned<Value>)>, Spanned<ParserError>> {
        let pos = self.lexer.pos();
        match self.parse_key_value() {
            Ok(kv) => Ok(Some(kv)),
            Err((e, key)) => {
                if let Some(key) = key {
                    let span = key.back().unwrap().span;
                    self.error_handler
                        .handle(ParserError::IgnoringKey.spanned(span));
                }
                if self.lexer.pos() == pos {
                    Err(e)
                } else {
                    self.error_handler.handle(e);
                    Ok(None)
                }
            }
        }
    }

    #[expect(clippy::type_complexity)]
    fn parse_key_value(
        &mut self,
    ) -> Result<(Key, Spanned<Value>), (Spanned<ParserError>, Option<Key>)> {
        let key = self.parse_key();
        let eq = self.parse_exact(Token::Equals, true);
        let value = self.parse_value();
        let key = match key {
            Ok(k) => k,
            Err(e) => return Err((e, None)),
        };
        if let Err(e) = eq {
            return Err((e, Some(key)));
        }
        let value = match value {
            Ok(v) => v,
            Err(e) => return Err((e, Some(key))),
        };
        Ok((key, value))
    }

    fn parse_key(&mut self) -> Result<Key, Spanned<ParserError>> {
        let mut parts = Key::new();
        loop {
            if parts.len() > 0 {
                if self.parse_exact(Token::Dot, false).is_err() {
                    break;
                }
            }
            let Some(token) = self.lexer.peek(false) else {
                break;
            };
            let s = match token.value {
                Token::LiteralString(s) => s.as_bstr().to_string(),
                Token::CookedString(s) => self.cook_string(s),
                Token::Literal(l) => l.as_bstr().to_string(),
                _ => break,
            };
            parts.push_back(s.spanned(token.span));
            let _ = self.next(false);
        }
        if parts.is_empty() {
            Err(ParserError::MissingKey.spanned(self.next_span()))
        } else {
            Ok(parts)
        }
    }

    fn parse_exact(
        &mut self,
        token: Token<'a>,
        value_context: bool,
    ) -> Result<Span, Spanned<ParserError>> {
        let actual = match self.peek(value_context) {
            Ok(t) if t.value == token => {
                let _ = self.next(value_context);
                return Ok(t.span);
            }
            Ok(t) => t.value.name(value_context),
            Err(_) => "end of file",
        };
        let span = self.next_span();
        Err(ParserError::Expected(token.name(value_context), actual).spanned(span))
    }

    fn last_span(&self) -> Span {
        self.last_span.unwrap_or(Span { lo: 0, hi: 0 })
    }

    fn next_span(&mut self) -> Span {
        self.lexer.peek(false).map(|v| v.span).unwrap_or_else(|| {
            let hi = self.last_span().hi;
            Span { lo: hi, hi }
        })
    }

    fn skip_tree(&mut self, start: Token, end: Token) {
        let mut depth = 1;
        while let Ok(next) = self.next(false) {
            if next.value == start {
                depth += 1;
            } else if next.value == end {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
        }
    }
}

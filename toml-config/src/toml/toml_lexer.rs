use crate::toml::toml_span::{Span, Spanned, SpannedExt};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Token<'a> {
    Dot,
    Equals,
    Comma,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    LiteralString(&'a [u8]),
    CookedString(&'a [u8]),
    Literal(&'a [u8]),
}

impl<'a> Token<'a> {
    pub fn name(self, value_context: bool) -> &'static str {
        match self {
            Token::Dot => "`.`",
            Token::Equals => "`=`",
            Token::Comma => "`,`",
            Token::LeftBracket => "`[`",
            Token::RightBracket => "`]`",
            Token::LeftBrace => "`{`",
            Token::RightBrace => "`}`",
            Token::LiteralString(_) | Token::CookedString(_) => "a string",
            Token::Literal(_) if value_context => "a literal",
            Token::Literal(_) => "a key",
        }
    }
}

pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    peek: Option<Spanned<Token<'a>>>,
    peek_value_context: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            pos: 0,
            peek: None,
            peek_value_context: false,
        }
    }

    pub fn pos(&mut self) -> usize {
        self.skip_ws();
        self.pos
    }

    fn skip_ws(&mut self) {
        while let Some(char) = self.input.get(self.pos).copied() {
            match char {
                b' ' | b'\t' | b'\n' => self.pos += 1,
                b'#' => {
                    self.pos += 1;
                    while let Some(char) = self.input.get(self.pos).copied() {
                        self.pos += 1;
                        if char == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    pub fn peek(&mut self, value_context: bool) -> Option<Spanned<Token<'a>>> {
        let next = self.next(value_context);
        self.peek = next;
        self.peek_value_context = value_context;
        next
    }

    pub fn next(&mut self, value_context: bool) -> Option<Spanned<Token<'a>>> {
        if let Some(peek) = self.peek.take() {
            if self.peek_value_context == value_context {
                return Some(peek);
            }
            self.pos = peek.span.lo;
        }

        use Token::*;

        macro_rules! get {
            ($off:expr) => {
                self.input.get(self.pos + $off).copied()
            };
        }

        self.skip_ws();

        let Some(c) = get!(0) else {
            return None;
        };
        let pos = self.pos;

        macro_rules! span {
            () => {
                Span {
                    lo: pos,
                    hi: self.pos,
                }
            };
        }

        'simple: {
            let t = match c {
                b'.' => Dot,
                b',' => Comma,
                b'=' => Equals,
                b'[' => LeftBracket,
                b']' => RightBracket,
                b'{' => LeftBrace,
                b'}' => RightBrace,
                _ => break 'simple,
            };
            self.pos += 1;
            return Some(t.spanned(span!()));
        }

        macro_rules! try_string {
            ($delim:expr, $escaping:expr, $ident:ident) => {
                if c == $delim {
                    'ml_string: {
                        let delim = ($delim, Some($delim), Some($delim));
                        if (c, get!(1), get!(2)) != delim {
                            break 'ml_string;
                        }
                        self.pos += 3;
                        if get!(0) == Some(b'\n') {
                            self.pos += 1;
                        }
                        let start = self.pos;
                        let end = loop {
                            let c = match get!(0) {
                                Some(c) => c,
                                _ => break self.pos,
                            };
                            self.pos += 1;
                            if $escaping && c == b'\\' {
                                self.pos += 1;
                            } else if c == $delim {
                                if (c, get!(0), get!(1)) == delim && get!(2) != Some($delim) {
                                    self.pos += 2;
                                    break self.pos - 3;
                                }
                            }
                        };
                        return Some($ident(&self.input[start..end]).spanned(span!()));
                    }
                    self.pos += 1;
                    let start = self.pos;
                    let end = loop {
                        let c = match get!(0) {
                            Some(c) => c,
                            _ => break self.pos,
                        };
                        self.pos += 1;
                        if $escaping && c == b'\\' {
                            self.pos += 1;
                        } else if c == $delim {
                            break self.pos - 1;
                        }
                    };
                    return Some($ident(&self.input[start..end]).spanned(span!()));
                }
            };
        }

        try_string!(b'\'', false, LiteralString);
        try_string!(b'"', true, CookedString);

        let start = self.pos;
        while let Some(c) = get!(0) {
            match c {
                b' ' | b'\t' | b'\n' | b'#' | b',' | b'=' | b'{' | b'}' | b'[' | b']' => break,
                b'.' if !value_context => break,
                _ => {}
            }
            self.pos += 1;
        }
        let end = self.pos;

        Some(Literal(&self.input[start..end]).spanned(span!()))
    }
}

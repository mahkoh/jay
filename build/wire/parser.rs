use anyhow::{Context, Result, bail};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TreeDelim {
    Paren,
    Brace,
}

impl TreeDelim {
    fn opening(self) -> u8 {
        match self {
            TreeDelim::Paren => b'(',
            TreeDelim::Brace => b'{',
        }
    }

    fn closing(self) -> u8 {
        match self {
            TreeDelim::Paren => b')',
            TreeDelim::Brace => b'}',
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Symbol {
    Comma,
    Colon,
    Equals,
}

impl Symbol {
    fn name(self) -> &'static str {
        match self {
            Symbol::Comma => "','",
            Symbol::Colon => "':'",
            Symbol::Equals => "'='",
        }
    }
}

#[derive(Debug)]
struct Token<'a> {
    line: u32,
    kind: TokenKind<'a>,
}

#[derive(Debug)]
enum TokenKind<'a> {
    Ident(&'a str),
    Num(u32),
    Tree {
        delim: TreeDelim,
        body: Vec<Token<'a>>,
    },
    Symbol(Symbol),
}

impl TokenKind<'_> {
    fn name(&self) -> &str {
        match self {
            TokenKind::Ident(_) => "identifier",
            TokenKind::Num(_) => "number",
            TokenKind::Tree { delim, .. } => match delim {
                TreeDelim::Paren => "'('-tree",
                TreeDelim::Brace => "'{'-tree",
            },
            TokenKind::Symbol(s) => s.name(),
        }
    }
}

#[derive(Copy, Clone)]
struct Cursor<'a> {
    pos: usize,
    s: &'a [u8],
}

impl Cursor<'_> {
    fn eof(&self) -> bool {
        self.pos >= self.s.len()
    }
}

fn tokenize<'a>(s: &'a [u8]) -> Result<Vec<Token<'a>>> {
    let mut tnz = Tokenizer {
        line: 1,
        cursor: Cursor { pos: 0, s },
        delim: None,
        res: vec![],
    };
    tnz.tokenize()?;
    Ok(tnz.res)
}

struct Tokenizer<'a> {
    line: u32,
    cursor: Cursor<'a>,
    delim: Option<TreeDelim>,
    res: Vec<Token<'a>>,
}

impl<'a> Tokenizer<'a> {
    fn tokenize_one(&mut self) -> Result<bool> {
        let c = &mut self.cursor;
        while !c.eof() {
            let b = c.s[c.pos];
            if matches!(b, b' ' | b'\n' | b'#') {
                c.pos += 1;
                if b == b'\n' {
                    self.line += 1;
                } else if b == b'#' {
                    while !c.eof() {
                        c.pos += 1;
                        if c.s[c.pos - 1] == b'\n' {
                            self.line += 1;
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
        if c.eof() {
            if self.delim.is_some() {
                bail!("Unexpected eof");
            }
            return Ok(false);
        }
        let line = self.line;
        let b = c.s[c.pos];
        let b_pos = c.pos;
        c.pos += 1;
        let kind = match b {
            b'a'..=b'z' => {
                while !c.eof() && matches!(c.s[c.pos], b'a'..=b'z' | b'_' | b'0'..=b'9') {
                    c.pos += 1;
                }
                TokenKind::Ident(std::str::from_utf8(&c.s[b_pos..c.pos])?)
            }
            b'0'..=b'9' => {
                c.pos -= 1;
                let mut num = 0;
                while !c.eof() && matches!(c.s[c.pos], b'0'..=b'9') {
                    num = num * 10 + (c.s[c.pos] - b'0') as u32;
                    c.pos += 1;
                }
                TokenKind::Num(num)
            }
            b',' => TokenKind::Symbol(Symbol::Comma),
            b'=' => TokenKind::Symbol(Symbol::Equals),
            b':' => TokenKind::Symbol(Symbol::Colon),
            b'(' => self.tokenize_tree(TreeDelim::Paren)?,
            b'{' => self.tokenize_tree(TreeDelim::Brace)?,
            c @ (b')' | b'}') => {
                if self.delim.map(|d| d.closing()) != Some(c) {
                    bail!("Unexpected '{}' in line {}", c, self.line);
                }
                return Ok(false);
            }
            _ => bail!("Unexpected byte {:?} in line {}", b as char, self.line),
        };
        self.res.push(Token { line, kind });
        Ok(true)
    }

    fn tokenize(&mut self) -> Result<()> {
        while self.tokenize_one()? {
            // nothing
        }
        Ok(())
    }

    fn tokenize_tree(&mut self, delim: TreeDelim) -> Result<TokenKind<'a>> {
        let mut tnz = Tokenizer {
            line: self.line,
            cursor: self.cursor,
            delim: Some(delim),
            res: vec![],
        };
        tnz.tokenize().with_context(|| {
            format!(
                "While tokenizing {:?} block starting in line {}",
                delim.opening() as char,
                self.line
            )
        })?;
        self.cursor.pos = tnz.cursor.pos;
        self.line = tnz.line;
        Ok(TokenKind::Tree {
            delim,
            body: tnz.res,
        })
    }
}

#[derive(Debug)]
pub struct Lined<T> {
    #[expect(dead_code)]
    pub line: u32,
    pub val: T,
}

#[derive(Debug)]
pub enum Type {
    Id(#[allow(dead_code)] String, String),
    U32,
    I32,
    U64,
    U64Rev,
    Str,
    OptStr,
    BStr,
    Fixed,
    Fd,
    Array(Box<Type>),
    Pod(String),
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Lined<Type>,
    #[allow(dead_code)]
    pub attribs: FieldAttribs,
}

#[derive(Debug)]
pub struct Message {
    pub name: String,
    pub camel_name: String,
    pub safe_name: String,
    pub id: u32,
    pub fields: Vec<Lined<Field>>,
    pub attribs: MessageAttribs,
    pub has_reference_type: bool,
}

#[derive(Debug, Default)]
pub struct MessageAttribs {
    pub since: Option<u32>,
    pub destructor: bool,
}

#[derive(Debug, Default)]
pub struct FieldAttribs {
    pub new: bool,
    pub nullable: bool,
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

#[derive(Debug)]
pub struct ParseResult {
    pub requests: Vec<Lined<Message>>,
    pub events: Vec<Lined<Message>>,
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<ParseResult> {
        let mut requests = vec![];
        let mut events = vec![];
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            let res = match ty.as_bytes() {
                b"request" => &mut requests,
                b"event" => &mut events,
                _ => bail!("In line {}: Unexpected entry {:?}", line, ty),
            };
            res.push(self.parse_message(res.len() as _)?);
        }
        Ok(ParseResult { requests, events })
    }

    fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    fn not_eof(&self) -> Result<()> {
        if self.eof() {
            bail!("Unexpected eof");
        }
        Ok(())
    }

    fn yes_eof(&self) -> Result<()> {
        if !self.eof() {
            bail!(
                "Unexpected trailing tokens in line {}",
                self.tokens[self.pos].line
            );
        }
        Ok(())
    }

    fn parse_message_attribs(&mut self, attribs: &mut MessageAttribs) -> Result<()> {
        let (_, tokens) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser { pos: 0, tokens };
        while !parser.eof() {
            let (line, name) = parser.expect_ident()?;
            match name {
                "since" => {
                    parser.expect_symbol(Symbol::Equals)?;
                    attribs.since = Some(parser.expect_number()?.1)
                }
                "destructor" => {
                    attribs.destructor = true;
                }
                _ => bail!("In line {}: Unexpected attribute {}", line, name),
            }
            if !parser.eof() {
                parser.expect_symbol(Symbol::Comma)?;
            }
        }
        Ok(())
    }

    fn parse_message(&mut self, id: u32) -> Result<Lined<Message>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.not_eof()?;
            let mut attribs = MessageAttribs::default();
            if let TokenKind::Tree {
                delim: TreeDelim::Paren,
                ..
            } = self.tokens[self.pos].kind
            {
                self.parse_message_attribs(&mut attribs)?;
            }
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser {
                pos: 0,
                tokens: body,
            };
            let mut fields = vec![];
            while !parser.eof() {
                fields.push(parser.parse_field()?);
            }
            let has_reference_type = fields.iter().any(|f| match &f.val.ty.val {
                Type::OptStr | Type::Str | Type::BStr | Type::Array(..) => true,
                _ => false,
            });
            let safe_name = match name {
                "move" => "move_",
                "type" => "type_",
                "drop" => "drop_",
                "id" => "id_",
                _ => name,
            };
            Ok(Lined {
                line,
                val: Message {
                    name: name.to_owned(),
                    camel_name: to_camel(name),
                    safe_name: safe_name.to_string(),
                    id,
                    fields,
                    attribs,
                    has_reference_type,
                },
            })
        })();
        res.with_context(|| format!("While parsing message starting at line {}", line))
    }

    fn parse_field_attribs(&mut self, attribs: &mut FieldAttribs) -> Result<()> {
        let (_, tokens) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser { pos: 0, tokens };
        while !parser.eof() {
            let (line, name) = parser.expect_ident()?;
            match name {
                "new" => attribs.new = true,
                "nullable" => attribs.nullable = true,
                _ => bail!("In line {}: Unexpected attribute {}", line, name),
            }
            if !parser.eof() {
                parser.expect_symbol(Symbol::Comma)?;
            }
        }
        Ok(())
    }

    fn parse_field(&mut self) -> Result<Lined<Field>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Colon)?;
            let ty = self.parse_type()?;
            let mut attribs = FieldAttribs::default();
            if !self.eof() {
                if let TokenKind::Tree {
                    delim: TreeDelim::Paren,
                    ..
                } = self.tokens[self.pos].kind
                {
                    self.parse_field_attribs(&mut attribs)?;
                }
            }
            if !self.eof() {
                self.expect_symbol(Symbol::Comma)?;
            }
            Ok(Lined {
                line,
                val: Field {
                    name: name.to_owned(),
                    ty,
                    attribs,
                },
            })
        })();
        res.with_context(|| format!("While parsing field starting at line {}", line))
    }

    fn expect_ident(&mut self) -> Result<(u32, &'a str)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Ident(id) => Ok((token.line, *id)),
            k => bail!(
                "In line {}: Expected identifier, found {}",
                token.line,
                k.name()
            ),
        }
    }

    fn expect_number(&mut self) -> Result<(u32, u32)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Num(n) => Ok((token.line, *n)),
            k => bail!(
                "In line {}: Expected number, found {}",
                token.line,
                k.name()
            ),
        }
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<()> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Symbol(s) if *s == symbol => Ok(()),
            k => bail!(
                "In line {}: Expected {}, found {}",
                token.line,
                symbol.name(),
                k.name()
            ),
        }
    }

    fn expect_tree_(&mut self) -> Result<(u32, TreeDelim, &'a [Token<'a>])> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Tree { delim, body } => Ok((token.line, *delim, body)),
            k => bail!("In line {}: Expected tree, found {}", token.line, k.name()),
        }
    }

    fn expect_tree(&mut self, exp_delim: TreeDelim) -> Result<(u32, &'a [Token<'a>])> {
        let (line, delim, tokens) = self.expect_tree_()?;
        if delim == exp_delim {
            Ok((line, tokens))
        } else {
            bail!(
                "In line {}: Expected {:?}-delimited tree, found {:?}-delimited tree",
                line,
                exp_delim,
                delim.opening()
            )
        }
    }

    fn parse_rust_path(&mut self) -> Result<Lined<String>> {
        let mut path = String::new();
        let mut line = None;
        loop {
            self.not_eof()?;
            let (l, id) = self.expect_ident()?;
            if line.is_none() {
                line = Some(l);
            }
            path.push_str(id);
            if self.eof() {
                break;
            }
            self.expect_symbol(Symbol::Colon)?;
            self.expect_symbol(Symbol::Colon)?;
            path.push_str("::");
        }
        Ok(Lined {
            line: line.unwrap(),
            val: path,
        })
    }

    fn parse_type(&mut self) -> Result<Lined<Type>> {
        self.not_eof()?;
        let (line, ty) = self.expect_ident()?;
        let ty = match ty.as_bytes() {
            b"pod" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser {
                    pos: 0,
                    tokens: body,
                };
                let ty = parser.parse_rust_path().with_context(|| {
                    format!("While parsing pod element type starting in line {}", line)
                })?;
                Type::Pod(ty.val)
            }
            b"u64" => Type::U64,
            b"u64_rev" => Type::U64Rev,
            b"u32" => Type::U32,
            b"i32" => Type::I32,
            b"str" => Type::Str,
            b"optstr" => Type::OptStr,
            b"bstr" => Type::BStr,
            b"fixed" => Type::Fixed,
            b"fd" => Type::Fd,
            b"array" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let ty: Result<_> = (|| {
                    let mut parser = Parser {
                        pos: 0,
                        tokens: body,
                    };
                    let ty = parser.parse_type()?;
                    parser.yes_eof()?;
                    match &ty.val {
                        Type::Id(..) => {}
                        Type::U32 => {}
                        Type::I32 => {}
                        Type::U64 => {}
                        Type::U64Rev => {}
                        Type::Fixed => {}
                        Type::Pod(..) => {}
                        _ => {
                            bail!("Only numerical and pod types can be array elements");
                        }
                    }
                    Ok(ty)
                })();
                let ty = ty.with_context(|| {
                    format!("While parsing array element type starting in line {}", line)
                })?;
                Type::Array(Box::new(ty.val))
            }
            b"id" => {
                let (_, body) = self.expect_tree(TreeDelim::Paren)?;
                let ident: Result<_> = (|| {
                    let mut parser = Parser {
                        pos: 0,
                        tokens: body,
                    };
                    let id = parser.expect_ident()?;
                    parser.yes_eof()?;
                    Ok(id)
                })();
                let (_, ident) = ident.with_context(|| {
                    format!("While parsing identifier starting in line {}", line)
                })?;
                Type::Id(ident.to_owned(), to_camel(ident))
            }
            _ => bail!("Unknown type {}", ty),
        };
        Ok(Lined { line, val: ty })
    }
}

pub fn parse_messages(s: &[u8]) -> Result<ParseResult> {
    let tokens = tokenize(s)?;
    let mut parser = Parser {
        pos: 0,
        tokens: &tokens,
    };
    parser.parse()
}

pub fn to_camel(s: &str) -> String {
    let mut last_was_underscore = true;
    let mut res = String::new();
    for mut b in s.as_bytes().iter().copied() {
        if b == b'_' {
            last_was_underscore = true;
        } else {
            if last_was_underscore {
                b = b.to_ascii_uppercase()
            }
            res.push(b as char);
            last_was_underscore = false;
        }
    }
    res
}

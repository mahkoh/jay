use anyhow::{bail, Context, Result};
use bstr::{BStr, ByteSlice};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TreeDelim {
    Paren,
    Brace,
}

impl TreeDelim {
    pub fn opening(self) -> u8 {
        match self {
            TreeDelim::Paren => b'(',
            TreeDelim::Brace => b'{',
        }
    }

    pub fn closing(self) -> u8 {
        match self {
            TreeDelim::Paren => b')',
            TreeDelim::Brace => b'}',
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Symbol {
    Comma,
    Colon,
    Equals,
}

impl Symbol {
    pub fn name(self) -> &'static str {
        match self {
            Symbol::Comma => "','",
            Symbol::Colon => "':'",
            Symbol::Equals => "'='",
        }
    }
}

#[derive(Debug)]
pub struct Token<'a> {
    pub line: u32,
    pub kind: TokenKind<'a>,
}

#[derive(Debug)]
pub enum TokenKind<'a> {
    Ident(&'a BStr),
    Num(u32),
    Tree {
        delim: TreeDelim,
        body: Vec<Token<'a>>,
    },
    Symbol(Symbol),
}

impl TokenKind<'_> {
    pub fn name(&self) -> &str {
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

pub fn tokenize<'a>(s: &'a [u8]) -> Result<Vec<Token<'a>>> {
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
            b'a'..=b'z' | b'A'..=b'Z' => {
                while !c.eof()
                    && matches!(c.s[c.pos], b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9')
                {
                    c.pos += 1;
                }
                TokenKind::Ident(c.s[b_pos..c.pos].as_bstr())
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
    #[allow(dead_code)]
    pub line: u32,
    pub val: T,
}

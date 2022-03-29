use crate::open;
use crate::tokens::{tokenize, Symbol, Token, TokenKind, TreeDelim};
use anyhow::{bail, Context, Result};
use bstr::{BStr, BString, ByteSlice};
use std::cell::Cell;
use std::collections::HashMap;
use std::io::Write;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::rc::Rc;

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
    ext_idx: Option<usize>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token<'a>]) -> Self {
        Self {
            pos: 0,
            tokens,
            ext_idx: None,
        }
    }

    fn parse(&mut self, ext_idx: &mut usize) -> Result<Protocol> {
        let mut first = true;
        let mut res = Protocol {
            extension: None,
            structs: vec![],
            requests: vec![],
            bitmasks: vec![],
            enums: vec![],
            events: vec![],
            eventcopies: vec![],
        };
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            match ty.as_bytes() {
                b"ext" => {
                    if !first {
                        bail!("In line {}: ext must be the first directive", line);
                    }
                    res.extension = Some(self.parse_extension(line)?);
                    self.ext_idx = Some(*ext_idx);
                    *ext_idx += 1;
                }
                b"struct" => res.structs.push(self.parse_struct(line)?),
                b"request" => res.requests.push(self.parse_request(line)?),
                b"bitmask" => res.bitmasks.push(self.parse_bitmask(line)?),
                b"event" => res.events.push(self.parse_event(line, false)?),
                b"xge" => res.events.push(self.parse_event(line, true)?),
                b"eventcopy" => res.eventcopies.push(self.parse_event_copy(line)?),
                b"enum" => res.enums.push(self.parse_enum(line)?),
                _ => bail!("In line {}: Unexpected entry {:?}", line, ty),
            }
            first = false;
        }
        Ok(res)
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

    fn parse_extension(&mut self, line: u32) -> Result<BString> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_string()?;
            Ok(name.to_owned())
        })();
        res.with_context(|| format!("While parsing extension starting in line {}", line))
    }

    fn parse_request(&mut self, line: u32) -> Result<Request> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_ident()?;
            self.expect_symbol(Symbol::Equals)?;
            let (_, opcode) = self.expect_num()?;
            let (_, body) = self.expect_tree(TreeDelim::Paren)?;
            let mut parser = Parser::new(body);
            let request = parser.parse_struct_body(name)?;
            self.not_eof()?;
            let reply = if self.tokens[self.pos].kind == TokenKind::Symbol(Symbol::Semicolon) {
                self.pos += 1;
                None
            } else {
                let (_, body) = self.expect_tree(TreeDelim::Brace)?;
                let mut parser = Parser::new(body);
                Some(parser.parse_struct_body(name)?)
            };
            Ok(Request {
                opcode,
                ext_idx: self.ext_idx,
                request,
                reply,
            })
        })();
        res.with_context(|| format!("While parsing request starting at line {}", line))
    }

    fn parse_struct(&mut self, line: u32) -> Result<Struct> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_ident()?;
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser::new(body);
            parser.parse_struct_body(name)
        })();
        res.with_context(|| format!("While parsing struct starting at line {}", line))
    }

    fn parse_event_copy(&mut self, line: u32) -> Result<EventCopy> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_ident()?;
            self.expect_symbol(Symbol::Equals)?;
            let (_, opcode) = self.expect_num()?;
            self.expect_symbol(Symbol::Equals)?;
            let (_, original) = self.expect_ident()?;
            self.expect_symbol(Symbol::Semicolon)?;
            Ok(EventCopy {
                name: name.to_owned(),
                opcode,
                original: original.to_owned(),
            })
        })();
        res.with_context(|| format!("While parsing eventcopy starting at line {}", line))
    }

    fn parse_event(&mut self, line: u32, xge: bool) -> Result<Event> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_ident()?;
            self.expect_symbol(Symbol::Equals)?;
            let (_, opcode) = self.expect_num()?;
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser::new(body);
            let data = parser.parse_struct_body(name)?;
            Ok(Event {
                opcode,
                xge,
                data,
                ext_idx: self.ext_idx,
            })
        })();
        res.with_context(|| format!("While parsing event starting at line {}", line))
    }

    fn parse_enum(&mut self, line: u32) -> Result<Enum> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_ident()?;
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser::new(body);
            let mut variants = vec![];
            while !parser.eof() {
                let (_, name) = parser.expect_ident()?;
                parser.expect_symbol(Symbol::Colon)?;
                let ty = parser.parse_type()?;
                parser.expect_symbol(Symbol::Equals)?;
                let (_, value) = parser.expect_num()?;
                if !parser.eof() {
                    parser.expect_symbol(Symbol::Comma)?;
                }
                variants.push(EnumVariant {
                    name: name.to_owned(),
                    ty,
                    value,
                });
            }
            Ok(Enum {
                name: name.to_owned(),
                variants,
            })
        })();
        res.with_context(|| format!("While parsing enum starting at line {}", line))
    }

    fn parse_bitmask(&mut self, line: u32) -> Result<Bitmask> {
        let res: Result<_> = (|| {
            let (_, name) = self.expect_ident()?;
            let (_, body) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser::new(body);
            let mut variants = vec![];
            while !parser.eof() {
                let (_, name) = parser.expect_ident()?;
                parser.expect_symbol(Symbol::Colon)?;
                let ty = parser.parse_type()?;
                parser.expect_symbol(Symbol::Equals)?;
                let (_, bit) = parser.expect_num()?;
                if !parser.eof() {
                    parser.expect_symbol(Symbol::Comma)?;
                }
                variants.push(BitmaskVariant {
                    name: name.to_owned(),
                    ty,
                    bit,
                });
            }
            Ok(Bitmask {
                name: name.to_owned(),
                variants,
            })
        })();
        res.with_context(|| format!("While parsing bitmask starting at line {}", line))
    }

    fn parse_struct_body(&mut self, name: &BStr) -> Result<Struct> {
        let mut fields = vec![];
        while !self.eof() {
            fields.push(self.parse_field()?);
        }
        Ok(Struct {
            name: name.to_owned(),
            fields,
            needs_lt: Cell::new(None),
            has_fds: Cell::new(None),
        })
    }

    fn parse_field(&mut self) -> Result<Field> {
        self.not_eof()?;
        let line = self.tokens[self.pos].line;
        let res: Result<_> = (|| {
            let field = if self.tokens[self.pos].kind == TokenKind::Symbol(Symbol::At) {
                self.pos += 1;
                let (_, name) = self.expect_ident()?;
                match name.as_bytes() {
                    b"pad" => Field::Pad(self.expect_num()?.1),
                    b"align" => Field::Align(self.expect_num()?.1),
                    _ => bail!("Unexpected directive {}", name),
                }
            } else {
                let (_, name) = self.expect_ident()?;
                self.expect_symbol(Symbol::Colon)?;
                let ty = self.parse_type()?;
                let mut value = None;
                if !self.eof() {
                    if self.tokens[self.pos].kind == TokenKind::Symbol(Symbol::Equals) {
                        self.pos += 1;
                        value = Some(self.parse_expr()?);
                    }
                }
                Field::Real(RealField {
                    name: name.to_owned(),
                    ty,
                    value,
                })
            };
            if !self.eof() {
                self.expect_symbol(Symbol::Comma)?;
            }
            Ok(field)
        })();
        res.with_context(|| format!("While parsing field starting at line {}", line))
    }

    fn expect_num(&mut self) -> Result<(u32, u32)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Num(id) => Ok((token.line, *id)),
            k => bail!(
                "In line {}: Expected number, found {}",
                token.line,
                k.name()
            ),
        }
    }

    fn expect_ident(&mut self) -> Result<(u32, &'a BStr)> {
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

    fn expect_string(&mut self) -> Result<(u32, &'a BString)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::String(id) => Ok((token.line, id)),
            k => bail!(
                "In line {}: Expected identifier, found {}",
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

    fn parse_type(&mut self) -> Result<Type> {
        let (_, ty) = self.expect_ident()?;
        let ty = match ty.as_bytes() {
            b"i8" => Type::I8,
            b"u8" => Type::U8,
            b"i16" => Type::I16,
            b"u16" => Type::U16,
            b"i32" => Type::I32,
            b"u32" => Type::U32,
            b"i64" => Type::I64,
            b"u64" => Type::U64,
            b"fd" => Type::Fd,
            b"str" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser::new(body);
                let ty: Result<_> = (|| {
                    let len = parser.parse_expr()?;
                    if !parser.eof() {
                        bail!("Trailing tokens in str length");
                    }
                    Ok(Type::String(len))
                })();
                ty.with_context(|| format!("While parsing string starting in line {}", line))?
            }
            b"list" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser::new(body);
                let ty: Result<_> = (|| {
                    let ty = parser.parse_type()?;
                    let mut len = None;
                    if !parser.eof() {
                        parser.expect_symbol(Symbol::Comma)?;
                        len = Some(parser.parse_expr()?);
                        if !parser.eof() {
                            bail!("Trailing tokens in list type");
                        }
                    }
                    Ok(Type::List(Box::new(ty), len))
                })();
                ty.with_context(|| format!("While parsing list starting in line {}", line))?
            }
            b"bitmask" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser::new(body);
                let ty: Result<_> = (|| {
                    let (_, name) = parser.expect_ident()?;
                    parser.expect_symbol(Symbol::Comma)?;
                    let len = parser.parse_expr()?;
                    if !parser.eof() {
                        bail!("Trailing tokens in bitmask type");
                    }
                    Ok(Type::Bitmask(name.to_owned(), len))
                })();
                ty.with_context(|| format!("While parsing bitmask starting in line {}", line))?
            }
            b"enum" => {
                let (line, body) = self.expect_tree(TreeDelim::Paren)?;
                let mut parser = Parser::new(body);
                let ty: Result<_> = (|| {
                    let ty = parser.parse_type()?;
                    parser.expect_symbol(Symbol::Comma)?;
                    let len = parser.parse_expr()?;
                    if !parser.eof() {
                        bail!("Trailing tokens in enum type");
                    }
                    Ok(Type::Enum(Box::new(ty), len))
                })();
                ty.with_context(|| format!("While parsing enum starting in line {}", line))?
            }
            _ => Type::Named(ty.to_owned()),
        };
        Ok(ty)
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        let (line, expr) = self.expect_ident()?;
        let res: Result<_> = (|| {
            let expr = match expr.as_bytes() {
                b"len" => Expr::Len(self.parse_field_name_expr()?),
                b"field" => Expr::Field(self.parse_field_name_expr()?),
                b"literal" => Expr::Literal(self.parse_literal_expr()?),
                b"bitmask" => Expr::Bitmask(self.parse_field_name_expr()?),
                b"variant" => Expr::Variant(self.parse_field_name_expr()?),
                b"sum" => Expr::Sum(self.parse_sum_expr()?),
                b"iter" => Expr::Iter(self.parse_iter_expr()?),
                b"it" => Expr::It,
                b"popcount" => Expr::Popcount(self.parse_popcount_expr()?),
                b"div" => {
                    let (left, right) = self.parse_div_expr()?;
                    Expr::Div(left, right)
                }
                b"plus" => {
                    let (left, right) = self.parse_plus_expr()?;
                    Expr::Plus(left, right)
                }
                b"map" => {
                    let (iter, fun) = self.parse_map_expr()?;
                    Expr::Map(iter, fun)
                }
                b"mul" => {
                    let (left, right) = self.parse_mul_expr()?;
                    Expr::Mul(left, right)
                }
                _ => bail!("Unknown expression {}", expr),
            };
            Ok(expr)
        })();
        res.with_context(|| format!("While parsing expression starting in line {}", line))
    }

    fn parse_literal_expr(&mut self) -> Result<u32> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let (_, name) = parser.expect_num()?;
        if !parser.eof() {
            bail!("Trailing tokens in literal");
        }
        Ok(name)
    }

    fn parse_field_name_expr(&mut self) -> Result<BString> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let (_, name) = parser.expect_ident()?;
        if !parser.eof() {
            bail!("Trailing tokens in field name");
        }
        Ok(name.to_owned())
    }

    fn parse_sum_expr(&mut self) -> Result<Box<Expr>> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let expr = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in sum body");
        }
        Ok(Box::new(expr))
    }

    fn parse_iter_expr(&mut self) -> Result<Box<Expr>> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let expr = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in iter body");
        }
        Ok(Box::new(expr))
    }

    fn parse_popcount_expr(&mut self) -> Result<Box<Expr>> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let expr = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in popcount body");
        }
        Ok(Box::new(expr))
    }

    fn parse_mul_expr(&mut self) -> Result<(Box<Expr>, Box<Expr>)> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let left = parser.parse_expr()?;
        parser.expect_symbol(Symbol::Comma)?;
        let right = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in mul body");
        }
        Ok((Box::new(left), Box::new(right)))
    }

    fn parse_map_expr(&mut self) -> Result<(Box<Expr>, Box<Expr>)> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let iter = parser.parse_expr()?;
        parser.expect_symbol(Symbol::Comma)?;
        let fun = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in map body");
        }
        Ok((Box::new(iter), Box::new(fun)))
    }

    fn parse_div_expr(&mut self) -> Result<(Box<Expr>, Box<Expr>)> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let iter = parser.parse_expr()?;
        parser.expect_symbol(Symbol::Comma)?;
        let fun = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in div body");
        }
        Ok((Box::new(iter), Box::new(fun)))
    }

    fn parse_plus_expr(&mut self) -> Result<(Box<Expr>, Box<Expr>)> {
        let (_, tree) = self.expect_tree(TreeDelim::Paren)?;
        let mut parser = Parser::new(tree);
        let iter = parser.parse_expr()?;
        parser.expect_symbol(Symbol::Comma)?;
        let fun = parser.parse_expr()?;
        if !parser.eof() {
            bail!("Trailing tokens in plus body");
        }
        Ok((Box::new(iter), Box::new(fun)))
    }
}

fn needs_lifetime(ty: &Type, protocols: &Protocols) -> Result<bool> {
    let res = match ty {
        Type::I8 => false,
        Type::U8 => false,
        Type::I16 => false,
        Type::U16 => false,
        Type::I32 => false,
        Type::U32 => false,
        Type::I64 => false,
        Type::U64 => false,
        Type::Fd => false,
        Type::List(_, _) => true,
        Type::Named(n) => named_needs_lt(n.as_bstr(), protocols)?,
        Type::String(_) => true,
        Type::Bitmask(_, _) => false,
        Type::Enum(n, _) => needs_lifetime(n, protocols)?,
    };
    Ok(res)
}

fn named_needs_lt(name: &BStr, protocols: &Protocols) -> Result<bool> {
    let s = match protocols.types_by_name.get(name) {
        Some(s) => s,
        _ => bail!("Struct {} referenced but not defined", name),
    };
    match s {
        NamedType::Struct(s) => struct_needs_lt(s, protocols),
        NamedType::Bitmask(_) => Ok(false),
        NamedType::Enum(s) => enum_needs_lt(s, protocols),
    }
}

fn enum_needs_lt(s: &Enum, protocols: &Protocols) -> Result<bool> {
    for v in &s.variants {
        if needs_lifetime(&v.ty, protocols)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn struct_needs_lt(s: &Struct, protocols: &Protocols) -> Result<bool> {
    if let Some(lt) = s.needs_lt.get() {
        return Ok(lt);
    }
    let mut needs_lt = false;
    for field in &s.fields {
        if let Field::Real(f) = field {
            if f.value.is_none() {
                if needs_lifetime(&f.ty, protocols)? {
                    needs_lt = true;
                    break;
                }
            }
        }
    }
    s.needs_lt.set(Some(needs_lt));
    Ok(needs_lt)
}

fn type_is_pod(ty: &Type) -> bool {
    match ty {
        Type::I8 => true,
        Type::U8 => true,
        Type::I16 => true,
        Type::U16 => true,
        Type::I32 => true,
        Type::U32 => true,
        Type::I64 => true,
        Type::U64 => true,
        Type::String(_) => false,
        Type::Fd => false,
        Type::List(..) => false,
        Type::Named(..) => false,
        Type::Bitmask(..) => false,
        Type::Enum(..) => false,
    }
}

fn type_has_fds(ty: &Type, protocols: &Protocols) -> Result<bool> {
    let res = match ty {
        Type::I8 => false,
        Type::U8 => false,
        Type::I16 => false,
        Type::U16 => false,
        Type::I32 => false,
        Type::U32 => false,
        Type::I64 => false,
        Type::U64 => false,
        Type::String(_) => false,
        Type::Fd => true,
        Type::List(ty, _) => return type_has_fds(ty, protocols),
        Type::Named(n) => return named_has_fds(n.as_bstr(), protocols),
        Type::Bitmask(_, _) => false,
        Type::Enum(n, _) => return type_has_fds(n, protocols),
    };
    Ok(res)
}

fn named_has_fds(s: &BStr, protocols: &Protocols) -> Result<bool> {
    let s = match protocols.types_by_name.get(s) {
        Some(s) => s,
        _ => bail!("Struct {} referenced but not defined", s),
    };
    match s {
        NamedType::Struct(s) => struct_has_fds(s, protocols),
        NamedType::Bitmask(_) => Ok(false),
        NamedType::Enum(e) => enum_has_fds(e, protocols),
    }
}

fn enum_has_fds(s: &Enum, protocols: &Protocols) -> Result<bool> {
    for v in &s.variants {
        if type_has_fds(&v.ty, protocols)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn struct_has_fds(s: &Struct, protocols: &Protocols) -> Result<bool> {
    if let Some(lt) = s.has_fds.get() {
        return Ok(lt);
    }
    let mut has_fds = false;
    for field in &s.fields {
        if let Field::Real(f) = field {
            if type_has_fds(&f.ty, protocols)? {
                has_fds = true;
                break;
            }
        }
    }
    s.has_fds.set(Some(has_fds));
    Ok(has_fds)
}

#[derive(Debug, Clone)]
enum Type {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    Fd,
    String(Expr),
    List(Box<Type>, Option<Expr>),
    Bitmask(BString, Expr),
    Enum(Box<Type>, Expr),
    Named(BString),
}

#[derive(Debug, Clone)]
enum Expr {
    It,
    Field(BString),
    Len(BString),
    Literal(u32),
    Bitmask(BString),
    Variant(BString),
    Sum(Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Map(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Plus(Box<Expr>, Box<Expr>),
    Iter(Box<Expr>),
    Popcount(Box<Expr>),
}

#[derive(Debug, Clone)]
struct RealField {
    name: BString,
    ty: Type,
    value: Option<Expr>,
}

#[derive(Debug, Clone)]
enum Field {
    Pad(u32),
    Align(u32),
    Real(RealField),
    Opcode(u32),
    ExtMajor,
}

#[derive(Debug)]
struct Struct {
    name: BString,
    fields: Vec<Field>,
    needs_lt: Cell<Option<bool>>,
    has_fds: Cell<Option<bool>>,
}

#[derive(Debug)]
struct BitmaskVariant {
    name: BString,
    ty: Type,
    bit: u32,
}

#[derive(Debug)]
struct Bitmask {
    name: BString,
    variants: Vec<BitmaskVariant>,
}

#[derive(Debug)]
struct EnumVariant {
    name: BString,
    ty: Type,
    value: u32,
}

#[derive(Debug)]
struct Enum {
    name: BString,
    variants: Vec<EnumVariant>,
}

#[derive(Debug)]
struct Event {
    opcode: u32,
    xge: bool,
    data: Struct,
    ext_idx: Option<usize>,
}

#[derive(Debug)]
struct EventCopy {
    name: BString,
    opcode: u32,
    original: BString,
}

#[derive(Debug)]
struct Request {
    opcode: u32,
    ext_idx: Option<usize>,
    request: Struct,
    reply: Option<Struct>,
}

#[derive(Debug)]
struct Protocol {
    extension: Option<BString>,
    structs: Vec<Struct>,
    requests: Vec<Request>,
    bitmasks: Vec<Bitmask>,
    enums: Vec<Enum>,
    events: Vec<Event>,
    eventcopies: Vec<EventCopy>,
}

#[derive(Debug)]
struct Extension {
    name: BString,
    ident: BString,
}

#[derive(Debug)]
enum NamedType {
    Struct(Rc<Struct>),
    Bitmask(Rc<Bitmask>),
    Enum(Rc<Enum>),
}

#[derive(Debug)]
struct Protocols {
    extensions: Vec<Extension>,
    structs: Vec<Rc<Struct>>,
    bitmasks: Vec<Rc<Bitmask>>,
    enums: Vec<Rc<Enum>>,
    requests: Vec<Request>,
    types_by_name: HashMap<BString, NamedType>,
    events_by_name: HashMap<BString, Rc<Event>>,
    events: Vec<Rc<Event>>,
    eventcopies: Vec<EventCopy>,
}

fn parse_protocol(path: &str, ext_idx: &mut usize) -> Result<Protocol> {
    let s = std::fs::read_to_string(path)?;
    let tokens = tokenize(s.as_bytes())?;
    Parser::new(&tokens).parse(ext_idx)
}

fn write_type<F: Write>(f: &mut F, ty: &Type, protocols: &Protocols) -> Result<()> {
    let ty = match ty {
        Type::I8 => "i8",
        Type::U8 => "u8",
        Type::I16 => "i16",
        Type::U16 => "u16",
        Type::I32 => "i32",
        Type::U32 => "u32",
        Type::I64 => "i64",
        Type::U64 => "u64",
        Type::String(_) => "&'a BStr",
        Type::Fd => "Rc<OwnedFd>",
        Type::Bitmask(n, _) => {
            write!(f, "{}", n)?;
            return Ok(());
        }
        Type::Enum(n, _) => {
            write_type(f, n, protocols)?;
            return Ok(());
        }
        Type::List(ty, _) => {
            if type_is_pod(ty) {
                write!(f, "&'a [")?;
                write_type(f, ty, protocols)?;
                write!(f, "]")?;
            } else {
                write!(f, "Cow<'a, [")?;
                write_type(f, ty, protocols)?;
                write!(f, "]>")?;
            }
            return Ok(());
        }
        Type::Named(n) => {
            let lt = if named_needs_lt(n.as_bstr(), protocols)? {
                "<'a>"
            } else {
                ""
            };
            write!(f, "{n}{lt}")?;
            return Ok(());
        }
    };
    write!(f, "{}", ty)?;
    Ok(())
}

fn write_expr<F: Write>(f: &mut F, e: &Expr, prefix: &str) -> Result<()> {
    match e {
        Expr::Field(n) => write!(f, "{prefix}{}", n)?,
        Expr::Len(n) => write!(f, "{prefix}{}.len()", n)?,
        Expr::Literal(n) => write!(f, "{}", n)?,
        Expr::Bitmask(n) => write!(f, "{prefix}{}.bitmask()", n)?,
        Expr::Variant(n) => write!(f, "{prefix}{}.variant()", n)?,
        Expr::Sum(e) => {
            write_expr(f, e, prefix)?;
            write!(f, ".reduce(|a, b| a + b).unwrap_or(0)")?;
        }
        Expr::Map(iter, map) => {
            write_expr(f, iter, prefix)?;
            write!(f, ".map(|val| ")?;
            write_expr(f, map, "val.")?;
            write!(f, ")")?;
        }
        Expr::Iter(ex) => {
            write_expr(f, ex, prefix)?;
            write!(f, ".iter()")?;
        }
        Expr::It => write!(f, "val")?,
        Expr::Popcount(ex) => {
            write_expr(f, ex, prefix)?;
            write!(f, ".count_ones()")?;
        }
        Expr::Div(l, r) | Expr::Plus(l, r) | Expr::Mul(l, r) => {
            write!(f, "(")?;
            write_expr(f, l, prefix)?;
            let c = match e {
                Expr::Div(..) => "/",
                Expr::Plus(..) => "+",
                Expr::Mul(..) => "*",
                _ => unreachable!(),
            };
            write!(f, " as u32 {} ", c)?;
            write_expr(f, r, prefix)?;
            write!(f, " as u32)")?;
        }
    }
    Ok(())
}

#[derive(Copy, Clone)]
enum StructUsecase<'a> {
    None,
    Request {
        request: &'a Request,
    },
    Reply,
    Event {
        xge: bool,
    },
    EventCopy {
        copy: &'a EventCopy,
        original: &'a Struct,
        xge: bool,
    },
}

fn format_xevent<F: Write>(
    f: &mut F,
    name: &BStr,
    s: &Struct,
    opcode: u32,
    protocols: &Protocols,
    ext: Option<usize>,
) -> Result<()> {
    let lt_a = if struct_needs_lt(s, protocols)? {
        "<'a>"
    } else {
        ""
    };
    writeln!(f)?;
    writeln!(f, "impl<'a> XEvent<'a> for {}{lt_a} {{", name)?;
    writeln!(f, "    const EXTENSION: Option<usize> = {:?};", ext)?;
    writeln!(f, "    const OPCODE: u16 = {:?};", opcode)?;
    writeln!(f, "}}")?;
    Ok(())
}

fn format_event<F: Write>(f: &mut F, s: &Event, protocols: &Protocols) -> Result<()> {
    format_struct(f, &s.data, protocols, &StructUsecase::Event { xge: s.xge })?;
    format_xevent(
        f,
        s.data.name.as_bstr(),
        &s.data,
        s.opcode,
        protocols,
        s.ext_idx,
    )
}

fn format_eventcopy<F: Write>(f: &mut F, s: &EventCopy, protocols: &Protocols) -> Result<()> {
    let original = match protocols.events_by_name.get(s.original.as_bstr()) {
        Some(o) => o,
        _ => bail!("Event {} referenced but not defined", s.original),
    };
    format_struct(
        f,
        &original.data,
        protocols,
        &StructUsecase::EventCopy {
            copy: s,
            original: &original.data,
            xge: original.xge,
        },
    )?;
    format_xevent(
        f,
        s.name.as_bstr(),
        &original.data,
        s.opcode,
        protocols,
        original.ext_idx,
    )
}

fn format_request<F: Write>(f: &mut F, s: &Request, protocols: &Protocols) -> Result<()> {
    format_struct(
        f,
        &s.request,
        protocols,
        &StructUsecase::Request { request: s },
    )?;
    let mut reply_has_lt = false;
    if let Some(reply) = &s.reply {
        reply_has_lt = struct_needs_lt(reply, protocols)?;
        format_struct(f, reply, protocols, &StructUsecase::Reply)?;
    }
    let lt_a = if struct_needs_lt(&s.request, protocols)? {
        "<'a>"
    } else {
        ""
    };
    writeln!(f)?;
    writeln!(f, "impl<'a> Request<'a> for {}{lt_a} {{", s.request.name)?;
    write!(f, "    type Reply = ")?;
    if s.reply.is_some() {
        let lt_static = if reply_has_lt { "<'static>" } else { "" };
        writeln!(f, "{}Reply{};", s.request.name, lt_static)?;
    } else {
        writeln!(f, "();")?;
    }
    writeln!(f, "    const EXTENSION: Option<usize> = {:?};", s.ext_idx)?;
    writeln!(f, "    const IS_VOID: bool = {};", s.reply.is_none())?;
    writeln!(f, "}}")?;
    Ok(())
}

enum FieldGroup {
    Pods { len: u32, fields: Vec<Field> },
    Single(Field),
}

fn create_groups(s: &Struct, usecase: &StructUsecase) -> Vec<FieldGroup> {
    let mut fields = &s.fields[..];
    let mut res = vec![];
    let mut current_len = 0;
    let mut current = vec![];
    let flush_current =
        |res: &mut Vec<FieldGroup>, current: &mut Vec<Field>, current_len: &mut u32| {
            if !current.is_empty() {
                res.push(FieldGroup::Pods {
                    len: mem::take(current_len),
                    fields: mem::take(current),
                });
            }
        };
    match usecase {
        StructUsecase::None => {}
        StructUsecase::Request { request } => {
            if request.ext_idx.is_some() {
                current.push(Field::ExtMajor);
                current.push(Field::Opcode(request.opcode));
                current.push(Field::Pad(2));
            } else {
                current.push(Field::Opcode(request.opcode));
                if let Some((first, rest)) = fields.split_first() {
                    fields = rest;
                    current.push(first.clone());
                    current.push(Field::Pad(2));
                } else {
                    current.push(Field::Pad(3));
                }
            }
            current_len = 4;
        }
        StructUsecase::Reply => {
            if let Some((first, rest)) = fields.split_first() {
                fields = rest;
                current.push(Field::Pad(1));
                current.push(first.clone());
                current.push(Field::Pad(6));
            } else {
                current.push(Field::Pad(8));
            }
            current_len = 8;
        }
        StructUsecase::Event { xge } => {
            current_len = 4;
            if *xge {
                current.push(Field::Pad(10));
                current_len = 10;
            } else if let Some((first, rest)) = fields.split_first() {
                fields = rest;
                current.push(Field::Pad(1));
                current.push(first.clone());
                current.push(Field::Pad(2));
            } else {
                current.push(Field::Pad(4));
            }
        }
        StructUsecase::EventCopy { .. } => unreachable!(),
    }
    for field in fields {
        match field {
            Field::Pad(n) => {
                current.push(field.clone());
                current_len += n;
            }
            Field::Align(_) => {
                flush_current(&mut res, &mut current, &mut current_len);
                res.push(FieldGroup::Single(field.clone()));
            }
            Field::Real(rf) => match rf.ty {
                Type::I8 | Type::U8 => {
                    current.push(field.clone());
                    current_len += 1;
                }
                Type::I16 | Type::U16 => {
                    current.push(field.clone());
                    current_len += 2;
                }
                Type::I32 | Type::U32 => {
                    current.push(field.clone());
                    current_len += 4;
                }
                Type::I64 | Type::U64 => {
                    current.push(field.clone());
                    current_len += 8;
                }
                _ => {
                    flush_current(&mut res, &mut current, &mut current_len);
                    res.push(FieldGroup::Single(field.clone()));
                }
            },
            Field::Opcode(_) => unreachable!(),
            Field::ExtMajor => unreachable!(),
        }
    }
    flush_current(&mut res, &mut current, &mut current_len);
    res
}

fn format_enum<F: Write>(f: &mut F, s: &Enum, protocols: &Protocols) -> Result<()> {
    let needs_lt = enum_needs_lt(s, protocols)?;
    let lt_a = if needs_lt { "<'a>" } else { "" };
    writeln!(f)?;
    writeln!(f, "#[derive(Debug, Clone)]")?;
    writeln!(f, "pub enum {}{lt_a} {{", s.name)?;
    for field in &s.variants {
        write!(f, "    {}(", field.name)?;
        write_type(f, &field.ty, protocols)?;
        writeln!(f, "),")?;
    }
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "impl{lt_a} {}{lt_a} {{", s.name)?;
    writeln!(f, "    pub fn variant(&self) -> u32 {{")?;
    writeln!(f, "        match self {{")?;
    for field in &s.variants {
        writeln!(
            f,
            "            Self::{}(..) => {},",
            field.name, field.value
        )?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    writeln!(f)?;
    writeln!(
        f,
        "    pub fn serialize(&self, formatter: &mut Formatter) {{"
    )?;
    writeln!(f, "        match self {{")?;
    for field in &s.variants {
        writeln!(
            f,
            "            Self::{}(v) => v.serialize(formatter),",
            field.name
        )?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    writeln!(f)?;
    writeln!(f, "    pub fn deserialize(parser: &mut Parser{lt_a}, value: u32) -> Result<Self, XconError> {{")?;
    writeln!(f, "        let res = match value {{")?;
    for field in &s.variants {
        writeln!(
            f,
            "            {} => Self::{}(parser.unmarshal()?),",
            field.value, field.name
        )?;
    }
    writeln!(
        f,
        "            _ => return Err(XconError::UnknownEnumVariant),"
    )?;
    writeln!(f, "        }};")?;
    writeln!(f, "        Ok(res)")?;
    writeln!(f, "    }}")?;
    writeln!(f, "}}")?;
    Ok(())
}

fn format_bitmask<F: Write>(f: &mut F, s: &Bitmask, protocols: &Protocols) -> Result<()> {
    writeln!(f)?;
    writeln!(f, "#[derive(Debug, Clone, Default)]")?;
    writeln!(f, "pub struct {} {{", s.name)?;
    for field in &s.variants {
        write!(f, "    pub {}: Option<", field.name)?;
        write_type(f, &field.ty, protocols)?;
        writeln!(f, ">,")?;
    }
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "impl {} {{", s.name)?;
    writeln!(f, "    pub fn bitmask(&self) -> u32 {{")?;
    writeln!(f, "        let mut res = 0;")?;
    for field in &s.variants {
        writeln!(
            f,
            "        res |= (self.{}.is_some() as u32) << {};",
            field.name, field.bit
        )?;
    }
    writeln!(f, "        res")?;
    writeln!(f, "    }}")?;
    writeln!(f)?;
    writeln!(
        f,
        "    pub fn serialize(&self, formatter: &mut Formatter) {{"
    )?;
    writeln!(f, "        let mut bytes = [0; {}];", s.variants.len() * 4)?;
    writeln!(f, "        let mut pos = 0;")?;
    for field in &s.variants {
        writeln!(f, "        if let Some(val) = self.{} {{", field.name)?;
        writeln!(
            f,
            "            bytes[pos..pos+4].copy_from_slice(&(val as u32).to_ne_bytes());"
        )?;
        writeln!(f, "            pos += 4;")?;
        writeln!(f, "        }}")?;
    }
    writeln!(f, "        formatter.write_bytes(&bytes[..pos]);")?;
    writeln!(f, "    }}")?;
    writeln!(f)?;
    writeln!(f, "    pub fn deserialize(&self, parser: &mut Parser, bitmask: u32) -> Result<Self, XconError> {{")?;
    writeln!(
        f,
        "        let b = parser.read_slice(bitmask.count_ones() as usize * 4)?;"
    )?;
    writeln!(f, "        let mut p = 0;")?;
    writeln!(f, "        Ok(Self {{")?;
    for field in &s.variants {
        writeln!(
            f,
            "            {}: if bitmask & (1 << {}) != 0 {{",
            field.name, field.bit
        )?;
        writeln!(
            f,
            "                let v = u32::from_ne_bytes([b[p], b[p+1], b[p+2], b[p+3]]);"
        )?;
        writeln!(f, "                p += 4;")?;
        writeln!(f, "                Some(v as _)")?;
        writeln!(f, "            }} else {{")?;
        writeln!(f, "                None")?;
        writeln!(f, "            }},")?;
    }
    writeln!(f, "        }})")?;
    writeln!(f, "    }}")?;
    writeln!(f, "}}")?;
    Ok(())
}

fn format_struct<F: Write>(
    f: &mut F,
    s: &Struct,
    protocols: &Protocols,
    usecase: &StructUsecase,
) -> Result<()> {
    let has_fds = struct_has_fds(s, protocols)?;
    let groups = match usecase {
        StructUsecase::EventCopy { .. } => vec![],
        _ => create_groups(s, usecase),
    };
    let struct_name = match usecase {
        StructUsecase::EventCopy { copy, .. } => format!("{}", copy.name),
        StructUsecase::Reply => format!("{}Reply", s.name),
        _ => s.name.to_string(),
    };
    let needs_lt = struct_needs_lt(s, protocols)?;
    let (lt_a, lt_b) = if needs_lt { ("<'a>", "<'b>") } else { ("", "") };
    writeln!(f)?;
    writeln!(f, "#[derive(Debug, Clone)]")?;
    writeln!(f, "pub struct {}{lt_a} {{", struct_name)?;
    if let StructUsecase::EventCopy { original, .. } = usecase {
        writeln!(f, "    pub data: {}{lt_a},", original.name)?;
    } else {
        for field in &s.fields {
            if let Field::Real(rf) = field {
                if rf.value.is_none() {
                    write!(f, "    pub {}: ", rf.name)?;
                    write_type(f, &rf.ty, protocols)?;
                    writeln!(f, ",")?;
                }
            }
        }
    }
    writeln!(f, "}}")?;
    if let StructUsecase::EventCopy { original, .. } = usecase {
        writeln!(f)?;
        writeln!(f, "impl{lt_a} std::ops::Deref for {}{lt_a} {{", struct_name)?;
        writeln!(f, "    type Target = {}{lt_a};", original.name)?;
        writeln!(f)?;
        writeln!(f, "    fn deref(&self) -> &Self::Target {{")?;
        writeln!(f, "        &self.data")?;
        writeln!(f, "    }}")?;
        writeln!(f, "}}")?;
    }
    writeln!(f)?;
    writeln!(
        f,
        "unsafe impl<'a> Message<'a> for {}{lt_a} {{",
        struct_name
    )?;
    writeln!(f, "    type Generic<'b> = {}{lt_b};", struct_name)?;
    writeln!(f, "    const IS_POD: bool = false;")?;
    writeln!(f, "    const HAS_FDS: bool = {has_fds};")?;
    let mut write_serialize = true;
    if matches!(
        usecase,
        StructUsecase::Reply
            | StructUsecase::Event { xge: true }
            | StructUsecase::EventCopy { xge: true, .. }
    ) {
        write_serialize = false;
    }
    if write_serialize {
        writeln!(f)?;
        writeln!(f, "    fn serialize(&self, formatter: &mut Formatter) {{")?;
        if let StructUsecase::EventCopy { .. } = usecase {
            writeln!(f, "        self.data.serialize(formatter);")?;
        } else {
            for group in &groups {
                match group {
                    FieldGroup::Pods { fields, .. } => {
                        writeln!(f, "        {{")?;
                        for field in fields {
                            if let Field::Real(rf) = field {
                                write!(f, "            let {}_bytes = ", rf.name)?;
                                match &rf.value {
                                    Some(e) => {
                                        writeln!(f, "{{")?;
                                        write!(f, "                let tmp: ")?;
                                        write_type(f, &rf.ty, protocols)?;
                                        write!(f, " = (")?;
                                        write_expr(f, e, "self.")?;
                                        writeln!(f, ") as _;")?;
                                        writeln!(f, "                tmp.to_ne_bytes()")?;
                                        writeln!(f, "            }};")?;
                                    }
                                    _ => writeln!(f, "self.{}.to_ne_bytes();", rf.name)?,
                                }
                            }
                        }
                        writeln!(f, "            formatter.write_bytes(&[")?;
                        for field in fields {
                            match field {
                                Field::Pad(n) => {
                                    write!(f, "               ")?;
                                    for _ in 0..*n {
                                        write!(f, " 0,")?;
                                    }
                                    writeln!(f)?;
                                }
                                Field::Real(rf) => {
                                    let num_bytes = match rf.ty {
                                        Type::I8 | Type::U8 => 1,
                                        Type::I16 | Type::U16 => 2,
                                        Type::I32 | Type::U32 => 4,
                                        Type::I64 | Type::U64 => 8,
                                        _ => unreachable!(),
                                    };
                                    write!(f, "               ")?;
                                    for i in 0..num_bytes {
                                        write!(f, " {}_bytes[{}],", rf.name, i)?;
                                    }
                                    writeln!(f)?;
                                }
                                Field::Opcode(n) => {
                                    writeln!(f, "                {},", n)?;
                                }
                                Field::ExtMajor => {
                                    writeln!(f, "                formatter.ext_opcode(),")?;
                                }
                                _ => unreachable!(),
                            }
                        }
                        writeln!(f, "            ]);")?;
                        writeln!(f, "        }}")?;
                    }
                    FieldGroup::Single(field) => match field {
                        Field::Align(n) => writeln!(f, "        formatter.align({n});")?,
                        Field::Real(rf) => match &rf.value {
                            Some(v) => {
                                writeln!(f, "        {{")?;
                                write!(f, "            let tmp: ")?;
                                write_type(f, &rf.ty, protocols)?;
                                write!(f, " = ")?;
                                write_expr(f, v, "self.")?;
                                writeln!(f, " as _;")?;
                                writeln!(f, "            tmp.serialize(formatter);")?;
                                writeln!(f, "        }}")?;
                            }
                            _ => writeln!(f, "        self.{}.serialize(formatter);", rf.name)?,
                        },
                        _ => unreachable!(),
                    },
                }
            }
        }
        writeln!(f, "    }}")?;
    }
    if !matches!(usecase, StructUsecase::Request { .. }) {
        writeln!(f)?;
        writeln!(
            f,
            "    fn deserialize(parser: &mut Parser<'a>) -> Result<Self, XconError> {{"
        )?;
        if let StructUsecase::EventCopy { original, .. } = usecase {
            writeln!(f, "        Ok(Self {{")?;
            writeln!(
                f,
                "            data: {}::deserialize(parser)?,",
                original.name
            )?;
            writeln!(f, "        }})")?;
        } else {
            for group in &groups {
                match group {
                    FieldGroup::Pods { len, fields } => {
                        writeln!(f, "        let bytes_ = parser.read_bytes::<{}>()?;", len)?;
                        let mut pos = 0;
                        for field in fields {
                            match field {
                                Field::Pad(n) => pos += n,
                                Field::Real(rf) => {
                                    write!(f, "        let {} = ", rf.name)?;
                                    write_type(f, &rf.ty, protocols)?;
                                    write!(f, "::from_ne_bytes([")?;
                                    let num_bytes = match rf.ty {
                                        Type::I8 | Type::U8 => 1,
                                        Type::I16 | Type::U16 => 2,
                                        Type::I32 | Type::U32 => 4,
                                        Type::I64 | Type::U64 => 8,
                                        _ => unreachable!(),
                                    };
                                    for i in 0..num_bytes {
                                        if i != 0 {
                                            write!(f, ", ")?;
                                        }
                                        write!(f, "bytes_[{}]", pos + i)?;
                                    }
                                    writeln!(f, "]);")?;
                                    pos += num_bytes;
                                }
                                Field::Opcode(_) => pos += 1,
                                Field::ExtMajor => pos += 1,
                                _ => unreachable!(),
                            }
                        }
                    }
                    FieldGroup::Single(field) => match field {
                        Field::Align(n) => writeln!(f, "        parser.align({n})?;")?,
                        Field::Real(rf) => {
                            write!(f, "        let {}: ", rf.name)?;
                            write_type(f, &rf.ty, protocols)?;
                            write!(f, " = ")?;
                            match &rf.ty {
                                Type::List(el, len) => {
                                    writeln!(f, "{{")?;
                                    write!(f, "            let len: Option<usize> = ")?;
                                    if let Some(len) = len {
                                        write!(f, "Some(")?;
                                        write_expr(f, len, "")?;
                                        writeln!(f, " as _);")?;
                                    } else {
                                        writeln!(f, "None;")?;
                                    }
                                    if type_is_pod(el) {
                                        writeln!(f, "            parser.read_list_slice(len)?")?;
                                    } else {
                                        writeln!(f, "            parser.read_list(len)?")?;
                                    }
                                    writeln!(f, "        }};")?;
                                }
                                Type::String(len) => {
                                    writeln!(f, "{{")?;
                                    write!(f, "            let len: usize = ")?;
                                    write_expr(f, len, "")?;
                                    writeln!(f, " as _;")?;
                                    writeln!(f, "            parser.read_string(len)?")?;
                                    writeln!(f, "        }};")?;
                                }
                                Type::Bitmask(n, bm) => {
                                    write!(f, "{}::deserialize(parser, ", n)?;
                                    write_expr(f, bm, "")?;
                                    writeln!(f, " as _)?;")?;
                                }
                                Type::Enum(n, bm) => {
                                    write!(f, "<")?;
                                    write_type(f, n, protocols)?;
                                    write!(f, ">::deserialize(parser, ")?;
                                    write_expr(f, bm, "")?;
                                    writeln!(f, " as _)?;")?;
                                }
                                _ => writeln!(f, "parser.unmarshal()?;")?,
                            }
                        }
                        _ => unreachable!(),
                    },
                }
            }
            writeln!(f, "        Ok(Self {{")?;
            for field in &s.fields {
                if let Field::Real(rf) = field {
                    if rf.value.is_none() {
                        writeln!(f, "            {},", rf.name)?;
                    }
                }
            }
            writeln!(f, "        }})")?;
        }
        writeln!(f, "    }}")?;
    }
    writeln!(f, "}}")?;
    Ok(())
}

pub fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=wire-xcon");

    let mut f = open("wire_xcon.rs")?;

    let mut files = vec![];
    for file in std::fs::read_dir("wire-xcon")? {
        files.push(file?.file_name());
    }
    files.sort();

    let mut protocols = Protocols {
        extensions: vec![],
        structs: vec![],
        bitmasks: vec![],
        enums: vec![],
        requests: vec![],
        types_by_name: Default::default(),
        events_by_name: Default::default(),
        events: vec![],
        eventcopies: vec![],
    };
    let mut ext_idx = 0;

    for file in files {
        let path = format!("wire-xcon/{}", file.as_bytes().as_bstr());
        let protocol = parse_protocol(&path, &mut ext_idx)
            .with_context(|| format!("While parsing {}", path))?;
        for s in protocol.structs {
            let s = Rc::new(s);
            protocols.structs.push(s.clone());
            protocols
                .types_by_name
                .insert(s.name.clone(), NamedType::Struct(s));
        }
        for s in protocol.bitmasks {
            let s = Rc::new(s);
            protocols.bitmasks.push(s.clone());
            protocols
                .types_by_name
                .insert(s.name.clone(), NamedType::Bitmask(s));
        }
        for s in protocol.enums {
            let s = Rc::new(s);
            protocols.enums.push(s.clone());
            protocols
                .types_by_name
                .insert(s.name.clone(), NamedType::Enum(s));
        }
        protocols.requests.extend(protocol.requests);
        for s in protocol.events {
            let s = Rc::new(s);
            protocols.events.push(s.clone());
            protocols.events_by_name.insert(s.data.name.clone(), s);
        }
        protocols.eventcopies.extend(protocol.eventcopies);
        if let Some(ext) = protocol.extension {
            let mut ident = vec![];
            for c in ext.as_bytes() {
                if matches!(*c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9') {
                    ident.push(*c);
                }
            }
            protocols.extensions.push(Extension {
                name: ext,
                ident: ident.into(),
            });
        }
    }

    writeln!(f, "#[derive(Copy, Clone, Debug, Eq, PartialEq)]")?;
    writeln!(f, "pub enum Extension {{")?;
    for ext in &protocols.extensions {
        writeln!(f, r#"    {},"#, ext.ident)?;
    }
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "impl Extension {{")?;
    writeln!(f, "    pub fn name(self) -> &'static str {{")?;
    writeln!(f, "        match self {{")?;
    for ext in &protocols.extensions {
        writeln!(f, r#"            Self::{} => "{}","#, ext.ident, ext.name)?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "pub const EXTENSIONS: &[Extension] = &[")?;
    for ext in &protocols.extensions {
        writeln!(f, r#"    Extension::{},"#, ext.ident)?;
    }
    writeln!(f, "];")?;

    for s in &protocols.bitmasks {
        format_bitmask(&mut f, s, &protocols)?;
    }

    for s in &protocols.structs {
        format_struct(&mut f, s, &protocols, &StructUsecase::None)?;
    }

    for s in &protocols.enums {
        format_enum(&mut f, s, &protocols)?;
    }

    for s in &protocols.requests {
        format_request(&mut f, s, &protocols)?;
    }

    for s in &protocols.events {
        format_event(&mut f, s, &protocols)?;
    }

    for s in &protocols.eventcopies {
        format_eventcopy(&mut f, s, &protocols)?;
    }

    Ok(())
}

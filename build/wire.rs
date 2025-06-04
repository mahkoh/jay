use {
    crate::open,
    anyhow::{Context, Result, bail},
    std::{fs::DirEntry, io::Write, os::unix::ffi::OsStrExt},
};

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
struct Lined<T> {
    #[expect(dead_code)]
    line: u32,
    val: T,
}

#[derive(Debug)]
enum Type {
    Id(String),
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
struct Field {
    name: String,
    ty: Lined<Type>,
}

#[derive(Debug)]
struct Message {
    name: String,
    camel_name: String,
    safe_name: String,
    id: u32,
    fields: Vec<Lined<Field>>,
    attribs: MessageAttribs,
    has_reference_type: bool,
}

#[derive(Debug, Default)]
struct MessageAttribs {
    since: Option<u32>,
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

struct ParseResult {
    requests: Vec<Lined<Message>>,
    events: Vec<Lined<Message>>,
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
            parser.expect_symbol(Symbol::Equals)?;
            match name {
                "since" => attribs.since = Some(parser.expect_number()?.1),
                _ => bail!("In line {}: Unexpected attribute {}", line, name),
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

    fn parse_field(&mut self) -> Result<Lined<Field>> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Colon)?;
            let ty = self.parse_type()?;
            if !self.eof() {
                self.expect_symbol(Symbol::Comma)?;
            }
            Ok(Lined {
                line,
                val: Field {
                    name: name.to_owned(),
                    ty,
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
                        Type::Id(_) => {}
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
                Type::Id(to_camel(ident))
            }
            _ => bail!("Unknown type {}", ty),
        };
        Ok(Lined { line, val: ty })
    }
}

fn parse_messages(s: &[u8]) -> Result<ParseResult> {
    let tokens = tokenize(s)?;
    let mut parser = Parser {
        pos: 0,
        tokens: &tokens,
    };
    parser.parse()
}

fn to_camel(s: &str) -> String {
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

fn write_type<W: Write>(f: &mut W, ty: &Type) -> Result<()> {
    match ty {
        Type::Id(id) => write!(f, "{}Id", id)?,
        Type::U32 => write!(f, "u32")?,
        Type::I32 => write!(f, "i32")?,
        Type::U64 => write!(f, "u64")?,
        Type::U64Rev => write!(f, "u64")?,
        Type::Str => write!(f, "&'a str")?,
        Type::OptStr => write!(f, "Option<&'a str>")?,
        Type::BStr => write!(f, "&'a BStr")?,
        Type::Fixed => write!(f, "Fixed")?,
        Type::Fd => write!(f, "Rc<OwnedFd>")?,
        Type::Array(n) => {
            write!(f, "&'a [")?;
            write_type(f, n)?;
            write!(f, "]")?;
        }
        Type::Pod(p) => f.write_all(p.as_bytes())?,
    }
    Ok(())
}

fn write_field<W: Write>(f: &mut W, field: &Field) -> Result<()> {
    write!(f, "        pub {}: ", field.name)?;
    write_type(f, &field.ty.val)?;
    writeln!(f, ",")?;
    Ok(())
}

fn write_message_type<W: Write>(
    f: &mut W,
    obj: &str,
    message: &Message,
    needs_lifetime: bool,
) -> Result<()> {
    let lifetime = if needs_lifetime { "<'a>" } else { "" };
    writeln!(f, "    pub struct {}{} {{", message.camel_name, lifetime)?;
    writeln!(f, "        pub self_id: {}Id,", obj)?;
    for field in &message.fields {
        write_field(f, &field.val)?;
    }
    writeln!(f, "    }}")?;
    writeln!(
        f,
        "    impl{} std::fmt::Debug for {}{} {{",
        lifetime, message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{"
    )?;
    write!(f, r#"            write!(fmt, "{}("#, message.name)?;
    for (i, field) in message.fields.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        let formatter = match &field.val.ty.val {
            Type::OptStr | Type::Str | Type::Fd | Type::Array(..) => "{:?}",
            _ => "{}",
        };
        write!(f, "{}: {}", field.val.name, formatter)?;
    }
    write!(f, r#")""#)?;
    for field in &message.fields {
        write!(f, ", self.{}", field.val.name)?;
    }
    writeln!(f, r")")?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_message<W: Write>(f: &mut W, obj: &str, message: &Message) -> Result<()> {
    let has_reference_type = message.has_reference_type;
    let uppercase = message.name.to_ascii_uppercase();
    writeln!(f)?;
    writeln!(f, "    pub const {}: u32 = {};", uppercase, message.id)?;
    write_message_type(f, obj, message, has_reference_type)?;
    let lifetime = if has_reference_type { "<'a>" } else { "" };
    let lifetime_b = if has_reference_type { "<'b>" } else { "" };
    let parser = if message.fields.len() > 0 {
        "parser"
    } else {
        "_parser"
    };
    writeln!(
        f,
        "    impl<'a> RequestParser<'a> for {}{} {{",
        message.camel_name, lifetime
    )?;
    writeln!(
        f,
        "        type Generic<'b> = {}{};",
        message.camel_name, lifetime_b,
    )?;
    writeln!(f, "        const ID: u32 = {};", message.id)?;
    writeln!(
        f,
        "        fn parse({}: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {{",
        parser
    )?;
    writeln!(f, "            Ok(Self {{")?;
    writeln!(f, "                self_id: {}Id::NONE,", obj)?;
    for field in &message.fields {
        let p = match &field.val.ty.val {
            Type::Id(_) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::U64 => "u64",
            Type::U64Rev => "u64_rev",
            Type::OptStr => "optstr",
            Type::Str => "str",
            Type::Fixed => "fixed",
            Type::Fd => "fd",
            Type::BStr => "bstr",
            Type::Array(_) => "binary_array",
            Type::Pod(_) => "binary",
        };
        writeln!(f, "                {}: parser.{}()?,", field.val.name, p)?;
    }
    writeln!(f, "            }})")?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    writeln!(
        f,
        "    impl{} EventFormatter for {}{} {{",
        lifetime, message.camel_name, lifetime
    )?;
    writeln!(f, "        fn format(self, fmt: &mut MsgFormatter<'_>) {{")?;
    writeln!(f, "            fmt.header(self.self_id, {});", uppercase)?;
    fn write_fmt_expr<W: Write>(f: &mut W, prefix: &str, ty: &Type, access: &str) -> Result<()> {
        let p = match ty {
            Type::Id(_) => "object",
            Type::U32 => "uint",
            Type::I32 => "int",
            Type::U64 => "u64",
            Type::U64Rev => "u64_rev",
            Type::OptStr => "optstr",
            Type::Str | Type::BStr => "string",
            Type::Fixed => "fixed",
            Type::Fd => "fd",
            Type::Array(..) => "binary",
            Type::Pod(..) => "binary",
        };
        let rf = match ty {
            Type::Pod(..) => "&",
            _ => "",
        };
        writeln!(f, "            {}fmt.{}({}{});", prefix, p, rf, access)?;
        Ok(())
    }
    for field in &message.fields {
        write_fmt_expr(
            f,
            "",
            &field.val.ty.val,
            &format!("self.{}", field.val.name),
        )?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "        fn id(&self) -> ObjectId {{")?;
    writeln!(f, "            self.self_id.into()")?;
    writeln!(f, "        }}")?;
    writeln!(f, "        fn interface(&self) -> Interface {{")?;
    writeln!(f, "            {}", obj)?;
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RequestHandlerDirection {
    Request,
    Event,
}

fn write_request_handler<W: Write>(
    f: &mut W,
    camel_obj_name: &str,
    messages: &[Lined<Message>],
    direction: RequestHandlerDirection,
) -> Result<()> {
    let snake_direction;
    let camel_direction;
    let parent;
    let parser;
    let error;
    let param;
    writeln!(f)?;
    match direction {
        RequestHandlerDirection::Request => {
            snake_direction = "request";
            camel_direction = "Request";
            parent = "crate::object::Object";
            parser = "crate::client::Client";
            error = "crate::client::ClientError";
            param = "req";
        }
        RequestHandlerDirection::Event => {
            snake_direction = "event";
            camel_direction = "Event";
            parent = "crate::wl_usr::usr_object::UsrObject";
            parser = "crate::wl_usr::UsrCon";
            error = "crate::wl_usr::UsrConError";
            param = "ev";
            writeln!(f, "    #[allow(clippy::allow_attributes, dead_code)]")?;
        }
    }
    writeln!(
        f,
        "    pub trait {camel_obj_name}{camel_direction}Handler: {parent} + Sized {{"
    )?;
    writeln!(f, "        type Error: std::error::Error;")?;
    for message in messages {
        let msg = &message.val;
        let lt = match msg.has_reference_type {
            true => "<'_>",
            false => "",
        };
        writeln!(f)?;
        writeln!(
            f,
            "        fn {}(&self, {param}: {}{lt}, _slf: &Rc<Self>) -> Result<(), Self::Error>;",
            msg.safe_name, msg.camel_name
        )?;
    }
    writeln!(f)?;
    writeln!(f, "        #[inline(always)]")?;
    writeln!(f, "        fn handle_{snake_direction}_impl(")?;
    writeln!(f, "            self: Rc<Self>,")?;
    writeln!(f, "            client: &{parser},")?;
    writeln!(f, "            req: u32,")?;
    writeln!(
        f,
        "            parser: crate::utils::buffd::MsgParser<'_, '_>,"
    )?;
    writeln!(f, "        ) -> Result<(), {error}> {{")?;
    if messages.is_empty() {
        writeln!(f, "            #![allow(unused_variables)]")?;
        writeln!(f, "            Err({error}::InvalidMethod)")?;
    } else {
        writeln!(f, "            let method;")?;
        writeln!(
            f,
            "            let error: Box<dyn std::error::Error> = match req {{"
        )?;
        for message in messages {
            let msg = &message.val;
            write!(f, "                {} ", msg.id)?;
            if let Some(since) = msg.attribs.since {
                write!(f, "if self.version() >= {since} ")?;
            }
            writeln!(f, "=> {{")?;
            writeln!(f, "                    method = \"{}\";", msg.name)?;
            writeln!(
                f,
                "                    match client.parse(&*self, parser) {{"
            )?;
            writeln!(
                f,
                "                        Ok(req) => match self.{}(req, &self) {{",
                msg.safe_name
            )?;
            writeln!(f, "                            Ok(()) => return Ok(()),")?;
            writeln!(f, "                            Err(e) => Box::new(e),")?;
            writeln!(f, "                        }},")?;
            writeln!(
                f,
                "                        Err(e) => Box::new(crate::client::ParserError(e)),"
            )?;
            writeln!(f, "                    }}")?;
            writeln!(f, "                }},")?;
        }
        writeln!(
            f,
            "                _ => return Err({error}::InvalidMethod),"
        )?;
        writeln!(f, "            }};")?;
        writeln!(f, "            Err({error}::MethodError {{")?;
        writeln!(f, "                interface: {camel_obj_name},")?;
        writeln!(f, "                id: self.id(),")?;
        writeln!(f, "                method,")?;
        writeln!(f, "                error,")?;
        writeln!(f, "            }})")?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    Ok(())
}

fn write_file<W: Write>(f: &mut W, file: &DirEntry) -> Result<()> {
    let file_name = file.file_name();
    let file_name = std::str::from_utf8(file_name.as_bytes())?;
    println!("cargo:rerun-if-changed=wire/{}", file_name);
    let obj_name = file_name.split(".").next().unwrap();
    let camel_obj_name = to_camel(obj_name);
    writeln!(f)?;
    writeln!(f, "id!({}Id);", camel_obj_name)?;
    writeln!(f)?;
    writeln!(
        f,
        "pub const {}: Interface = Interface(\"{}\");",
        camel_obj_name, obj_name
    )?;
    let contents = std::fs::read(file.path())?;
    let messages = parse_messages(&contents)?;
    writeln!(f)?;
    writeln!(f, "pub mod {} {{", obj_name)?;
    writeln!(f, "    use super::*;")?;
    for message in messages.requests.iter().chain(messages.events.iter()) {
        write_message(f, &camel_obj_name, &message.val)?;
    }
    write_request_handler(
        f,
        &camel_obj_name,
        &messages.requests,
        RequestHandlerDirection::Request,
    )?;
    write_request_handler(
        f,
        &camel_obj_name,
        &messages.events,
        RequestHandlerDirection::Event,
    )?;
    writeln!(f, "}}")?;
    Ok(())
}

pub fn main() -> Result<()> {
    let mut f = open("wire.rs")?;
    writeln!(f, "use std::rc::Rc;")?;
    writeln!(f, "use uapi::OwnedFd;")?;
    writeln!(f, "use bstr::BStr;")?;
    writeln!(f, "use crate::fixed::Fixed;")?;
    writeln!(f, "use crate::client::{{EventFormatter, RequestParser}};")?;
    writeln!(f, "use crate::object::{{ObjectId, Interface}};")?;
    writeln!(
        f,
        "use crate::utils::buffd::{{MsgFormatter, MsgParser, MsgParserError}};"
    )?;
    println!("cargo:rerun-if-changed=wire");
    let mut files = vec![];
    for file in std::fs::read_dir("wire")? {
        files.push(file?);
    }
    files.sort_by_key(|f| f.file_name());
    for file in files {
        write_file(&mut f, &file)
            .with_context(|| format!("While processing {}", file.path().display()))?;
    }
    Ok(())
}

use crate::token::{Keyword, Op, Span, StrPrefix, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub pos: u32,
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(src).run()
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    indents: Vec<u32>,
    paren: u32,
}

enum LineStart {
    Blank,
    Code,
    Eof,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            src,
            pos: 0,
            tokens: Vec::new(),
            indents: vec![0],
            paren: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn rest(&self) -> &str {
        &self.src[self.pos..]
    }

    fn at(&self) -> u32 {
        self.pos as u32
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    fn err(&self, message: String, pos: u32) -> LexError {
        LexError { message, pos }
    }

    fn push(&mut self, kind: TokenKind, start: u32) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.at()),
        });
    }

    fn run(mut self) -> Result<Vec<Token>, LexError> {
        let mut line_start = true;
        loop {
            if line_start && self.paren == 0 {
                match self.line_start()? {
                    LineStart::Blank => continue,
                    LineStart::Eof => break,
                    LineStart::Code => line_start = false,
                }
            }
            match self.peek() {
                None => break,
                Some('\n') | Some('\r') => {
                    let start = self.at();
                    self.consume_newline();
                    if self.paren == 0 {
                        self.push(TokenKind::Newline, start);
                        line_start = true;
                    }
                }
                Some('#') => self.skip_to_line_end(),
                Some('\\') if matches!(self.peek_at(1), Some('\n') | Some('\r')) => {
                    self.bump();
                    self.consume_newline();
                }
                Some(' ') | Some('\t') | Some('\u{0c}') => {
                    self.bump();
                }
                Some(c) => {
                    let start = self.at();
                    let kind = self.scan_token(c)?;
                    self.push(kind, start);
                }
            }
        }
        if !line_start {
            let p = self.at();
            self.push(TokenKind::Newline, p);
        }
        while *self.indents.last().unwrap() > 0 {
            self.indents.pop();
            let p = self.at();
            self.push(TokenKind::Dedent, p);
        }
        let p = self.at();
        self.push(TokenKind::Eof, p);
        Ok(self.tokens)
    }

    fn line_start(&mut self) -> Result<LineStart, LexError> {
        let mut col: u32 = 0;
        loop {
            match self.peek() {
                Some(' ') => {
                    col += 1;
                    self.bump();
                }
                Some('\t') => {
                    col = (col / 8 + 1) * 8;
                    self.bump();
                }
                _ => break,
            }
        }
        match self.peek() {
            None => Ok(LineStart::Eof),
            Some('\n') | Some('\r') => {
                self.consume_newline();
                Ok(LineStart::Blank)
            }
            Some('#') => {
                self.skip_to_line_end();
                self.consume_newline();
                Ok(LineStart::Blank)
            }
            Some(_) => {
                self.apply_indent(col)?;
                Ok(LineStart::Code)
            }
        }
    }

    fn apply_indent(&mut self, col: u32) -> Result<(), LexError> {
        let top = *self.indents.last().unwrap();
        if col > top {
            self.indents.push(col);
            let p = self.at();
            self.push(TokenKind::Indent, p);
        } else if col < top {
            while *self.indents.last().unwrap() > col {
                self.indents.pop();
                let p = self.at();
                self.push(TokenKind::Dedent, p);
            }
            if *self.indents.last().unwrap() != col {
                return Err(self.err("unindent does not match any outer level".into(), self.at()));
            }
        }
        Ok(())
    }

    fn consume_newline(&mut self) {
        match self.peek() {
            Some('\r') => {
                self.bump();
                if self.peek() == Some('\n') {
                    self.bump();
                }
            }
            Some('\n') => {
                self.bump();
            }
            _ => {}
        }
    }

    fn skip_to_line_end(&mut self) {
        while !matches!(self.peek(), None | Some('\n') | Some('\r')) {
            self.bump();
        }
    }

    fn scan_token(&mut self, c: char) -> Result<TokenKind, LexError> {
        if c == '_' || c.is_alphabetic() {
            return self.scan_name();
        }
        if c.is_ascii_digit() || (c == '.' && self.peek_at(1).is_some_and(|d| d.is_ascii_digit())) {
            return self.scan_number();
        }
        if c == '"' || c == '\'' {
            return self.scan_string(StrPrefix::default());
        }
        self.scan_operator()
    }

    fn scan_name(&mut self) -> Result<TokenKind, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '_' || c.is_alphanumeric() {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if matches!(self.peek(), Some('"') | Some('\'')) {
            if let Some(prefix) = string_prefix(&s) {
                return self.scan_string(prefix);
            }
        }
        match Keyword::from_ident(&s) {
            Some(kw) => Ok(TokenKind::Keyword(kw)),
            None => Ok(TokenKind::Name(s)),
        }
    }

    fn scan_number(&mut self) -> Result<TokenKind, LexError> {
        if self.peek() == Some('0') {
            if let Some(p) = self.peek_at(1) {
                let radix = match p {
                    'x' | 'X' => 16,
                    'o' | 'O' => 8,
                    'b' | 'B' => 2,
                    _ => 0,
                };
                if radix != 0 {
                    self.bump();
                    self.bump();
                    let digits = self.take_radix_digits(radix);
                    if digits.is_empty() {
                        return Err(self.err("missing digits in number".into(), self.at()));
                    }
                    return Ok(TokenKind::Int { digits, radix });
                }
            }
        }
        let mut s = String::new();
        let mut is_float = false;
        self.take_dec(&mut s);
        if self.peek() == Some('.') {
            is_float = true;
            s.push('.');
            self.bump();
            self.take_dec(&mut s);
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            let save = self.pos;
            self.bump();
            let mut exp = String::new();
            if matches!(self.peek(), Some('+') | Some('-')) {
                exp.push(self.bump().unwrap());
            }
            let mut digs = String::new();
            self.take_dec(&mut digs);
            if digs.is_empty() {
                self.pos = save;
            } else {
                is_float = true;
                s.push('e');
                s.push_str(&exp);
                s.push_str(&digs);
            }
        }
        if is_float {
            s.parse::<f64>()
                .map(TokenKind::Float)
                .map_err(|_| self.err(format!("invalid float literal {s:?}"), self.at()))
        } else if s.is_empty() {
            Err(self.err("invalid number".into(), self.at()))
        } else {
            Ok(TokenKind::Int {
                digits: s,
                radix: 10,
            })
        }
    }

    fn take_dec(&mut self, out: &mut String) {
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                out.push(c);
                self.bump();
            } else if c == '_' {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn take_radix_digits(&mut self, radix: u32) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '_' {
                self.bump();
            } else if c.is_digit(radix) {
                out.push(c);
                self.bump();
            } else {
                break;
            }
        }
        out
    }

    fn scan_string(&mut self, prefix: StrPrefix) -> Result<TokenKind, LexError> {
        let start = self.at();
        let quote = self.bump().unwrap();
        let triple = self.peek() == Some(quote) && self.peek_at(1) == Some(quote);
        if triple {
            self.bump();
            self.bump();
        }
        let mut value = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated string".into(), start)),
                Some(c) if c == quote => {
                    if !triple {
                        self.bump();
                        break;
                    }
                    if self.peek_at(1) == Some(quote) && self.peek_at(2) == Some(quote) {
                        self.bump();
                        self.bump();
                        self.bump();
                        break;
                    }
                    value.push(c);
                    self.bump();
                }
                Some('\n') | Some('\r') if !triple => {
                    return Err(self.err("unterminated string".into(), start));
                }
                Some('\\') => {
                    self.bump();
                    if prefix.raw {
                        value.push('\\');
                        if let Some(n) = self.bump() {
                            value.push(n);
                        }
                    } else {
                        self.scan_escape(&mut value, start)?;
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.bump();
                }
            }
        }
        Ok(TokenKind::Str { value, prefix })
    }

    fn scan_escape(&mut self, out: &mut String, start: u32) -> Result<(), LexError> {
        match self.bump() {
            None => return Err(self.err("unterminated string".into(), start)),
            Some('\n') => {}
            Some('\r') => {
                if self.peek() == Some('\n') {
                    self.bump();
                }
            }
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('"') => out.push('"'),
            Some('0') => out.push('\0'),
            Some('a') => out.push('\u{07}'),
            Some('b') => out.push('\u{08}'),
            Some('f') => out.push('\u{0c}'),
            Some('v') => out.push('\u{0b}'),
            Some('x') => self.push_hex_escape(out, 2, start)?,
            Some('u') => self.push_hex_escape(out, 4, start)?,
            Some('U') => self.push_hex_escape(out, 8, start)?,
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
        Ok(())
    }

    fn push_hex_escape(&mut self, out: &mut String, n: usize, start: u32) -> Result<(), LexError> {
        let mut code: u32 = 0;
        for _ in 0..n {
            let d = self
                .peek()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| self.err("invalid hex escape".into(), start))?;
            code = code * 16 + d;
            self.bump();
        }
        let ch =
            char::from_u32(code).ok_or_else(|| self.err("invalid unicode escape".into(), start))?;
        out.push(ch);
        Ok(())
    }

    fn scan_operator(&mut self) -> Result<TokenKind, LexError> {
        use Op::*;
        for (s, op) in [
            ("**=", DoubleStarEq),
            ("//=", DoubleSlashEq),
            ("<<=", LShiftEq),
            (">>=", RShiftEq),
            ("...", Ellipsis),
        ] {
            if self.rest().starts_with(s) {
                self.advance(s.len());
                return Ok(self.op(op));
            }
        }
        for (s, op) in [
            ("**", DoubleStar),
            ("//", DoubleSlash),
            ("<<", LShift),
            (">>", RShift),
            ("==", Eq),
            ("!=", NotEq),
            ("<=", Le),
            (">=", Ge),
            ("->", Arrow),
            (":=", Walrus),
            ("+=", PlusEq),
            ("-=", MinusEq),
            ("*=", StarEq),
            ("/=", SlashEq),
            ("%=", PercentEq),
            ("@=", AtEq),
            ("&=", AmpEq),
            ("|=", PipeEq),
            ("^=", CaretEq),
        ] {
            if self.rest().starts_with(s) {
                self.advance(s.len());
                return Ok(self.op(op));
            }
        }
        let start = self.at();
        let c = self.bump().unwrap();
        let op = match c {
            '+' => Plus,
            '-' => Minus,
            '*' => Star,
            '/' => Slash,
            '%' => Percent,
            '@' => At,
            '&' => Amp,
            '|' => Pipe,
            '^' => Caret,
            '~' => Tilde,
            '<' => Lt,
            '>' => Gt,
            '=' => Assign,
            '(' => LParen,
            ')' => RParen,
            '[' => LBracket,
            ']' => RBracket,
            '{' => LBrace,
            '}' => RBrace,
            ',' => Comma,
            ':' => Colon,
            ';' => Semicolon,
            '.' => Dot,
            _ => return Err(self.err(format!("unexpected character {c:?}"), start)),
        };
        Ok(self.op(op))
    }

    fn op(&mut self, op: Op) -> TokenKind {
        match op {
            Op::LParen | Op::LBracket | Op::LBrace => self.paren += 1,
            Op::RParen | Op::RBracket | Op::RBrace => self.paren = self.paren.saturating_sub(1),
            _ => {}
        }
        TokenKind::Op(op)
    }
}

fn string_prefix(s: &str) -> Option<StrPrefix> {
    let l = s.to_ascii_lowercase();
    if !matches!(
        l.as_str(),
        "r" | "b" | "u" | "f" | "rb" | "br" | "rf" | "fr"
    ) {
        return None;
    }
    Some(StrPrefix {
        raw: l.contains('r'),
        bytes: l.contains('b'),
        fstring: l.contains('f'),
    })
}

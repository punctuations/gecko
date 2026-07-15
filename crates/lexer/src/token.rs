#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Int { digits: String, radix: u32 },
    Float(f64),
    Str { value: String, prefix: StrPrefix },
    Name(String),
    Keyword(Keyword),
    Op(Op),
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StrPrefix {
    pub raw: bool,
    pub bytes: bool,
    pub fstring: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    False,
    None,
    True,
    And,
    As,
    Assert,
    Async,
    Await,
    Break,
    Class,
    Continue,
    Def,
    Del,
    Elif,
    Else,
    Except,
    Finally,
    For,
    From,
    Global,
    If,
    Import,
    In,
    Is,
    Lambda,
    Nonlocal,
    Not,
    Or,
    Pass,
    Raise,
    Return,
    Try,
    While,
    With,
    Yield,
}

impl Keyword {
    pub fn from_ident(s: &str) -> Option<Keyword> {
        use Keyword::*;
        Some(match s {
            "False" => False,
            "None" => None,
            "True" => True,
            "and" => And,
            "as" => As,
            "assert" => Assert,
            "async" => Async,
            "await" => Await,
            "break" => Break,
            "class" => Class,
            "continue" => Continue,
            "def" => Def,
            "del" => Del,
            "elif" => Elif,
            "else" => Else,
            "except" => Except,
            "finally" => Finally,
            "for" => For,
            "from" => From,
            "global" => Global,
            "if" => If,
            "import" => Import,
            "in" => In,
            "is" => Is,
            "lambda" => Lambda,
            "nonlocal" => Nonlocal,
            "not" => Not,
            "or" => Or,
            "pass" => Pass,
            "raise" => Raise,
            "return" => Return,
            "try" => Try,
            "while" => While,
            "with" => With,
            "yield" => Yield,
            _ => return Option::None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Plus,
    Minus,
    Star,
    Slash,
    DoubleSlash,
    Percent,
    DoubleStar,
    At,
    Amp,
    Pipe,
    Caret,
    Tilde,
    LShift,
    RShift,
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Assign,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    DoubleSlashEq,
    PercentEq,
    DoubleStarEq,
    AtEq,
    AmpEq,
    PipeEq,
    CaretEq,
    LShiftEq,
    RShiftEq,
    Walrus,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow,
    Ellipsis,
}

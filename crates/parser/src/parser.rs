use ast::{
    Alias, BinOp, BoolOp, CmpOp, Comprehension, ExceptHandler, Expr, Keyword as KwArg, Module,
    Param, Stmt, UnOp,
};
use lexer::{Keyword as Kw, LexError, Op, Span, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError {
            message: e.message,
            span: Span::new(e.pos, e.pos),
        }
    }
}

pub fn parse(src: &str) -> Result<Module, ParseError> {
    let tokens = lexer::tokenize(src)?;
    Parser { tokens, pos: 0 }.program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn kind_at(&self, n: usize) -> &TokenKind {
        let i = (self.pos + n).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn kind(&self) -> &TokenKind {
        self.kind_at(0)
    }

    fn span(&self) -> Span {
        self.tokens[self.pos.min(self.tokens.len() - 1)].span
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            span: self.span(),
        })
    }

    fn at_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    fn at_op(&self, op: Op) -> bool {
        matches!(self.kind(), TokenKind::Op(o) if *o == op)
    }

    fn at_kw(&self, kw: Kw) -> bool {
        matches!(self.kind(), TokenKind::Keyword(k) if *k == kw)
    }

    fn eat_op(&mut self, op: Op) -> bool {
        if self.at_op(op) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, kw: Kw) -> bool {
        if self.at_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, op: Op) -> Result<(), ParseError> {
        if self.eat_op(op) {
            Ok(())
        } else {
            self.error(format!("expected {op:?}"))
        }
    }

    fn expect_kw(&mut self, kw: Kw) -> Result<(), ParseError> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            self.error(format!("expected keyword {kw:?}"))
        }
    }

    fn expect_name(&mut self) -> Result<String, ParseError> {
        if let TokenKind::Name(n) = self.kind() {
            let n = n.clone();
            self.advance();
            Ok(n)
        } else {
            self.error("expected a name")
        }
    }

    fn expect_newline(&mut self) -> Result<(), ParseError> {
        match self.kind() {
            TokenKind::Newline => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            _ => self.error("expected end of line"),
        }
    }

    fn program(mut self) -> Result<Module, ParseError> {
        let mut body = Vec::new();
        while !self.at_eof() {
            if matches!(self.kind(), TokenKind::Newline) {
                self.advance();
                continue;
            }
            body.extend(self.statement()?);
        }
        Ok(Module { body })
    }

    fn statement(&mut self) -> Result<Vec<Stmt>, ParseError> {
        match self.kind() {
            TokenKind::Keyword(Kw::Def) => Ok(vec![self.funcdef(Vec::new())?]),
            TokenKind::Keyword(Kw::If) => Ok(vec![self.if_stmt()?]),
            TokenKind::Keyword(Kw::While) => Ok(vec![self.while_stmt()?]),
            TokenKind::Keyword(Kw::For) => Ok(vec![self.for_stmt()?]),
            TokenKind::Keyword(Kw::Try) => Ok(vec![self.try_stmt()?]),
            TokenKind::Keyword(Kw::Class) => Ok(vec![self.class_stmt(Vec::new())?]),
            TokenKind::Op(Op::At) => Ok(vec![self.decorated()?]),
            _ => self.simple_line(),
        }
    }

    fn simple_line(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = vec![self.simple_stmt()?];
        while self.eat_op(Op::Semicolon) {
            if matches!(self.kind(), TokenKind::Newline | TokenKind::Eof) {
                break;
            }
            stmts.push(self.simple_stmt()?);
        }
        self.expect_newline()?;
        Ok(stmts)
    }

    fn simple_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.kind() {
            TokenKind::Keyword(Kw::Pass) => {
                self.advance();
                Ok(Stmt::Pass)
            }
            TokenKind::Keyword(Kw::Break) => {
                self.advance();
                Ok(Stmt::Break)
            }
            TokenKind::Keyword(Kw::Continue) => {
                self.advance();
                Ok(Stmt::Continue)
            }
            TokenKind::Keyword(Kw::Raise) => {
                self.advance();
                if matches!(self.kind(), TokenKind::Newline | TokenKind::Eof)
                    || self.at_op(Op::Semicolon)
                {
                    Ok(Stmt::Raise(None))
                } else {
                    Ok(Stmt::Raise(Some(self.test()?)))
                }
            }
            TokenKind::Keyword(Kw::Import) => self.import_stmt(),
            TokenKind::Keyword(Kw::From) => self.import_from_stmt(),
            TokenKind::Keyword(Kw::Nonlocal) => {
                self.advance();
                let mut names = vec![self.expect_name()?];
                while self.eat_op(Op::Comma) {
                    names.push(self.expect_name()?);
                }
                Ok(Stmt::Nonlocal(names))
            }
            TokenKind::Keyword(Kw::Return) => {
                self.advance();
                if matches!(self.kind(), TokenKind::Newline | TokenKind::Eof)
                    || self.at_op(Op::Semicolon)
                {
                    Ok(Stmt::Return(None))
                } else {
                    Ok(Stmt::Return(Some(self.testlist()?)))
                }
            }
            _ => self.expr_or_assign(),
        }
    }

    fn import_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::Import)?;
        let mut names = Vec::new();
        loop {
            let name = self.module_name()?;
            let asname = if self.eat_kw(Kw::As) {
                Some(self.expect_name()?)
            } else {
                None
            };
            names.push(Alias { name, asname });
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        Ok(Stmt::Import(names))
    }

    fn import_from_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::From)?;
        let module = self.module_name()?;
        self.expect_kw(Kw::Import)?;
        if self.at_op(Op::Star) {
            return self.error("'from module import *' is not supported yet");
        }
        let parenthesized = self.eat_op(Op::LParen);
        let mut names = Vec::new();
        loop {
            let name = self.expect_name()?;
            let asname = if self.eat_kw(Kw::As) {
                Some(self.expect_name()?)
            } else {
                None
            };
            names.push(Alias { name, asname });
            if !self.eat_op(Op::Comma) {
                break;
            }
            if parenthesized && self.at_op(Op::RParen) {
                break;
            }
        }
        if parenthesized {
            self.expect_op(Op::RParen)?;
        }
        Ok(Stmt::ImportFrom { module, names })
    }

    fn module_name(&mut self) -> Result<String, ParseError> {
        let name = self.expect_name()?;
        if self.at_op(Op::Dot) {
            return self.error("dotted module names are not supported yet");
        }
        Ok(name)
    }

    fn expr_or_assign(&mut self) -> Result<Stmt, ParseError> {
        let first = self.testlist()?;
        if let Some(op) = self.aug_op() {
            self.advance();
            let value = self.testlist()?;
            return Ok(Stmt::AugAssign {
                target: first,
                op,
                value,
            });
        }
        if self.at_op(Op::Assign) {
            let mut nodes = vec![first];
            while self.eat_op(Op::Assign) {
                nodes.push(self.testlist()?);
            }
            let value = nodes.pop().unwrap();
            return Ok(Stmt::Assign {
                targets: nodes,
                value,
            });
        }
        Ok(Stmt::Expr(first))
    }

    fn aug_op(&self) -> Option<BinOp> {
        let TokenKind::Op(o) = self.kind() else {
            return None;
        };
        Some(match o {
            Op::PlusEq => BinOp::Add,
            Op::MinusEq => BinOp::Sub,
            Op::StarEq => BinOp::Mul,
            Op::SlashEq => BinOp::Div,
            Op::DoubleSlashEq => BinOp::FloorDiv,
            Op::PercentEq => BinOp::Mod,
            Op::DoubleStarEq => BinOp::Pow,
            Op::AtEq => BinOp::MatMul,
            Op::AmpEq => BinOp::BitAnd,
            Op::PipeEq => BinOp::BitOr,
            Op::CaretEq => BinOp::BitXor,
            Op::LShiftEq => BinOp::LShift,
            Op::RShiftEq => BinOp::RShift,
            _ => return None,
        })
    }

    fn decorated(&mut self) -> Result<Stmt, ParseError> {
        let mut decorators = Vec::new();
        while self.eat_op(Op::At) {
            decorators.push(self.test()?);
            self.expect_newline()?;
        }
        match self.kind() {
            TokenKind::Keyword(Kw::Def) => self.funcdef(decorators),
            TokenKind::Keyword(Kw::Class) => self.class_stmt(decorators),
            _ => self.error("expected a def or class after a decorator"),
        }
    }

    fn funcdef(&mut self, decorators: Vec<Expr>) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::Def)?;
        let name = self.expect_name()?;
        self.expect_op(Op::LParen)?;
        let mut params = Vec::new();
        while !self.at_op(Op::RParen) {
            let name = self.expect_name()?;
            let default = if self.eat_op(Op::Assign) {
                Some(self.test()?)
            } else {
                None
            };
            params.push(Param { name, default });
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RParen)?;
        let body = self.suite()?;
        Ok(Stmt::FunctionDef {
            name,
            params,
            body,
            decorators,
        })
    }

    fn if_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::If)?;
        self.if_tail()
    }

    fn if_tail(&mut self) -> Result<Stmt, ParseError> {
        let test = self.test()?;
        let body = self.suite()?;
        let orelse = if self.eat_kw(Kw::Elif) {
            vec![self.if_tail()?]
        } else if self.eat_kw(Kw::Else) {
            self.suite()?
        } else {
            Vec::new()
        };
        Ok(Stmt::If { test, body, orelse })
    }

    fn while_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::While)?;
        let test = self.test()?;
        let body = self.suite()?;
        let orelse = if self.eat_kw(Kw::Else) {
            self.suite()?
        } else {
            Vec::new()
        };
        Ok(Stmt::While { test, body, orelse })
    }

    fn for_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::For)?;
        let target = self.target_list()?;
        self.expect_kw(Kw::In)?;
        let iter = self.testlist()?;
        let body = self.suite()?;
        let orelse = if self.eat_kw(Kw::Else) {
            self.suite()?
        } else {
            Vec::new()
        };
        Ok(Stmt::For {
            target,
            iter,
            body,
            orelse,
        })
    }

    fn class_stmt(&mut self, decorators: Vec<Expr>) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::Class)?;
        let name = self.expect_name()?;
        let mut bases = Vec::new();
        if self.eat_op(Op::LParen) {
            while !self.at_op(Op::RParen) {
                bases.push(self.test()?);
                if !self.eat_op(Op::Comma) {
                    break;
                }
            }
            self.expect_op(Op::RParen)?;
        }
        let body = self.suite()?;
        Ok(Stmt::ClassDef {
            name,
            bases,
            body,
            decorators,
        })
    }

    fn try_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kw(Kw::Try)?;
        let body = self.suite()?;
        let mut handlers: Vec<ExceptHandler> = Vec::new();
        while self.eat_kw(Kw::Except) {
            if handlers.last().is_some_and(|h| h.typ.is_none()) {
                return self.error("default except clause must be last");
            }
            let typ = if self.at_op(Op::Colon) {
                None
            } else {
                Some(self.test()?)
            };
            let name = if self.eat_kw(Kw::As) {
                Some(self.expect_name()?)
            } else {
                None
            };
            if typ.is_none() && name.is_some() {
                return self.error("a bare except cannot bind a name");
            }
            let handler_body = self.suite()?;
            handlers.push(ExceptHandler {
                typ,
                name,
                body: handler_body,
            });
        }
        let orelse = if self.eat_kw(Kw::Else) {
            if handlers.is_empty() {
                return self.error("try/else needs at least one except clause");
            }
            self.suite()?
        } else {
            Vec::new()
        };
        let finalbody = if self.eat_kw(Kw::Finally) {
            self.suite()?
        } else {
            Vec::new()
        };
        if handlers.is_empty() && finalbody.is_empty() {
            return self.error("try needs an except or finally clause");
        }
        Ok(Stmt::Try {
            body,
            handlers,
            orelse,
            finalbody,
        })
    }

    fn suite(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect_op(Op::Colon)?;
        if matches!(self.kind(), TokenKind::Newline) {
            self.advance();
            if !matches!(self.kind(), TokenKind::Indent) {
                return self.error("expected an indented block");
            }
            self.advance();
            let mut body = Vec::new();
            while !matches!(self.kind(), TokenKind::Dedent) && !self.at_eof() {
                body.extend(self.statement()?);
            }
            if !matches!(self.kind(), TokenKind::Dedent) {
                return self.error("expected dedent");
            }
            self.advance();
            Ok(body)
        } else {
            self.simple_line()
        }
    }

    fn target_list(&mut self) -> Result<Expr, ParseError> {
        let first = self.postfix()?;
        if self.at_op(Op::Comma) {
            let mut elts = vec![first];
            while self.eat_op(Op::Comma) {
                if self.at_kw(Kw::In) {
                    break;
                }
                elts.push(self.postfix()?);
            }
            Ok(Expr::Tuple(elts))
        } else {
            Ok(first)
        }
    }

    fn testlist(&mut self) -> Result<Expr, ParseError> {
        let first = self.test()?;
        if self.at_op(Op::Comma) {
            let mut elts = vec![first];
            while self.eat_op(Op::Comma) {
                if self.at_testlist_end() {
                    break;
                }
                elts.push(self.test()?);
            }
            Ok(Expr::Tuple(elts))
        } else {
            Ok(first)
        }
    }

    fn at_testlist_end(&self) -> bool {
        matches!(self.kind(), TokenKind::Newline | TokenKind::Eof)
            || self.at_op(Op::Assign)
            || self.at_op(Op::Colon)
            || self.at_op(Op::RParen)
            || self.at_op(Op::RBracket)
            || self.at_op(Op::RBrace)
            || self.at_op(Op::Semicolon)
    }

    fn test(&mut self) -> Result<Expr, ParseError> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> Result<Expr, ParseError> {
        let first = self.and_expr()?;
        if self.at_kw(Kw::Or) {
            let mut values = vec![first];
            while self.eat_kw(Kw::Or) {
                values.push(self.and_expr()?);
            }
            Ok(Expr::Bool_ {
                op: BoolOp::Or,
                values,
            })
        } else {
            Ok(first)
        }
    }

    fn and_expr(&mut self) -> Result<Expr, ParseError> {
        let first = self.not_expr()?;
        if self.at_kw(Kw::And) {
            let mut values = vec![first];
            while self.eat_kw(Kw::And) {
                values.push(self.not_expr()?);
            }
            Ok(Expr::Bool_ {
                op: BoolOp::And,
                values,
            })
        } else {
            Ok(first)
        }
    }

    fn not_expr(&mut self) -> Result<Expr, ParseError> {
        if self.eat_kw(Kw::Not) {
            let operand = Box::new(self.not_expr()?);
            Ok(Expr::Unary {
                op: UnOp::Not,
                operand,
            })
        } else {
            self.comparison()
        }
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.bitor()?;
        let mut ops = Vec::new();
        let mut comparators = Vec::new();
        while let Some(op) = self.cmp_op() {
            ops.push(op);
            comparators.push(self.bitor()?);
        }
        if ops.is_empty() {
            Ok(left)
        } else {
            Ok(Expr::Compare {
                left: Box::new(left),
                ops,
                comparators,
            })
        }
    }

    fn cmp_op(&mut self) -> Option<CmpOp> {
        let op = match self.kind() {
            TokenKind::Op(Op::Eq) => CmpOp::Eq,
            TokenKind::Op(Op::NotEq) => CmpOp::NotEq,
            TokenKind::Op(Op::Lt) => CmpOp::Lt,
            TokenKind::Op(Op::Le) => CmpOp::LtE,
            TokenKind::Op(Op::Gt) => CmpOp::Gt,
            TokenKind::Op(Op::Ge) => CmpOp::GtE,
            TokenKind::Keyword(Kw::In) => CmpOp::In,
            TokenKind::Keyword(Kw::Is) => {
                self.advance();
                return Some(if self.eat_kw(Kw::Not) {
                    CmpOp::IsNot
                } else {
                    CmpOp::Is
                });
            }
            TokenKind::Keyword(Kw::Not) => {
                if matches!(self.kind_at(1), TokenKind::Keyword(Kw::In)) {
                    self.advance();
                    self.advance();
                    return Some(CmpOp::NotIn);
                }
                return None;
            }
            _ => return None,
        };
        self.advance();
        Some(op)
    }

    fn bitor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.bitxor()?;
        while self.eat_op(Op::Pipe) {
            left = bin(BinOp::BitOr, left, self.bitxor()?);
        }
        Ok(left)
    }

    fn bitxor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.bitand()?;
        while self.eat_op(Op::Caret) {
            left = bin(BinOp::BitXor, left, self.bitand()?);
        }
        Ok(left)
    }

    fn bitand(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.shift()?;
        while self.eat_op(Op::Amp) {
            left = bin(BinOp::BitAnd, left, self.shift()?);
        }
        Ok(left)
    }

    fn shift(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.arith()?;
        loop {
            let op = if self.eat_op(Op::LShift) {
                BinOp::LShift
            } else if self.eat_op(Op::RShift) {
                BinOp::RShift
            } else {
                break;
            };
            left = bin(op, left, self.arith()?);
        }
        Ok(left)
    }

    fn arith(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.term()?;
        loop {
            let op = if self.eat_op(Op::Plus) {
                BinOp::Add
            } else if self.eat_op(Op::Minus) {
                BinOp::Sub
            } else {
                break;
            };
            left = bin(op, left, self.term()?);
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.factor()?;
        loop {
            let op = if self.eat_op(Op::Star) {
                BinOp::Mul
            } else if self.eat_op(Op::Slash) {
                BinOp::Div
            } else if self.eat_op(Op::DoubleSlash) {
                BinOp::FloorDiv
            } else if self.eat_op(Op::Percent) {
                BinOp::Mod
            } else if self.eat_op(Op::At) {
                BinOp::MatMul
            } else {
                break;
            };
            left = bin(op, left, self.factor()?);
        }
        Ok(left)
    }

    fn factor(&mut self) -> Result<Expr, ParseError> {
        let op = if self.at_op(Op::Plus) {
            UnOp::Pos
        } else if self.at_op(Op::Minus) {
            UnOp::Neg
        } else if self.at_op(Op::Tilde) {
            UnOp::Invert
        } else {
            return self.power();
        };
        self.advance();
        Ok(Expr::Unary {
            op,
            operand: Box::new(self.factor()?),
        })
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let base = self.postfix()?;
        if self.eat_op(Op::DoubleStar) {
            Ok(bin(BinOp::Pow, base, self.factor()?))
        } else {
            Ok(base)
        }
    }

    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.atom()?;
        loop {
            if self.eat_op(Op::LParen) {
                e = self.call(e)?;
            } else if self.eat_op(Op::LBracket) {
                let index = Box::new(self.test()?);
                self.expect_op(Op::RBracket)?;
                e = Expr::Subscript {
                    value: Box::new(e),
                    index,
                };
            } else if self.eat_op(Op::Dot) {
                let attr = self.expect_name()?;
                e = Expr::Attribute {
                    value: Box::new(e),
                    attr,
                };
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn call(&mut self, func: Expr) -> Result<Expr, ParseError> {
        let mut args = Vec::new();
        let mut keywords = Vec::new();
        while !self.at_op(Op::RParen) {
            if let TokenKind::Name(n) = self.kind() {
                if matches!(self.kind_at(1), TokenKind::Op(Op::Assign)) {
                    let arg = n.clone();
                    self.advance();
                    self.advance();
                    let value = self.test()?;
                    keywords.push(KwArg { arg, value });
                    if !self.eat_op(Op::Comma) {
                        break;
                    }
                    continue;
                }
            }
            args.push(self.test()?);
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RParen)?;
        Ok(Expr::Call {
            func: Box::new(func),
            args,
            keywords,
        })
    }

    fn atom(&mut self) -> Result<Expr, ParseError> {
        match self.kind().clone() {
            TokenKind::Int { digits, radix } => {
                self.advance();
                Ok(Expr::Int { digits, radix })
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr::Float(f))
            }
            TokenKind::Str { value, .. } => {
                self.advance();
                let mut s = value;
                while let TokenKind::Str { value: next, .. } = self.kind().clone() {
                    self.advance();
                    s.push_str(&next);
                }
                Ok(Expr::Str(s))
            }
            TokenKind::Keyword(Kw::True) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::Keyword(Kw::False) => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Keyword(Kw::None) => {
                self.advance();
                Ok(Expr::None)
            }
            TokenKind::Name(n) => {
                self.advance();
                Ok(Expr::Name(n))
            }
            TokenKind::Op(Op::LParen) => {
                self.advance();
                self.paren_group()
            }
            TokenKind::Op(Op::LBracket) => {
                self.advance();
                self.list_literal()
            }
            TokenKind::Op(Op::LBrace) => {
                self.advance();
                self.dict_literal()
            }
            _ => self.error("expected an expression"),
        }
    }

    fn comprehensions(&mut self) -> Result<Vec<Comprehension>, ParseError> {
        let mut out = Vec::new();
        while self.eat_kw(Kw::For) {
            let target = self.target_list()?;
            self.expect_kw(Kw::In)?;
            let iter = self.test()?;
            let mut ifs = Vec::new();
            while self.eat_kw(Kw::If) {
                ifs.push(self.test()?);
            }
            out.push(Comprehension { target, iter, ifs });
        }
        Ok(out)
    }

    fn paren_group(&mut self) -> Result<Expr, ParseError> {
        if self.eat_op(Op::RParen) {
            return Ok(Expr::Tuple(Vec::new()));
        }
        let first = self.test()?;
        if self.at_kw(Kw::For) {
            let generators = self.comprehensions()?;
            self.expect_op(Op::RParen)?;
            return Ok(Expr::GeneratorExp {
                elt: Box::new(first),
                generators,
            });
        }
        if self.at_op(Op::Comma) {
            let mut elts = vec![first];
            while self.eat_op(Op::Comma) {
                if self.at_op(Op::RParen) {
                    break;
                }
                elts.push(self.test()?);
            }
            self.expect_op(Op::RParen)?;
            Ok(Expr::Tuple(elts))
        } else {
            self.expect_op(Op::RParen)?;
            Ok(first)
        }
    }

    fn list_literal(&mut self) -> Result<Expr, ParseError> {
        if self.eat_op(Op::RBracket) {
            return Ok(Expr::List(Vec::new()));
        }
        let first = self.test()?;
        if self.at_kw(Kw::For) {
            let generators = self.comprehensions()?;
            self.expect_op(Op::RBracket)?;
            return Ok(Expr::ListComp {
                elt: Box::new(first),
                generators,
            });
        }
        let mut elts = vec![first];
        while self.eat_op(Op::Comma) {
            if self.at_op(Op::RBracket) {
                break;
            }
            elts.push(self.test()?);
        }
        self.expect_op(Op::RBracket)?;
        Ok(Expr::List(elts))
    }

    fn dict_literal(&mut self) -> Result<Expr, ParseError> {
        if self.eat_op(Op::RBrace) {
            return Ok(Expr::Dict(Vec::new()));
        }
        let key = self.test()?;
        self.expect_op(Op::Colon)?;
        let value = self.test()?;
        if self.at_kw(Kw::For) {
            let generators = self.comprehensions()?;
            self.expect_op(Op::RBrace)?;
            return Ok(Expr::DictComp {
                key: Box::new(key),
                value: Box::new(value),
                generators,
            });
        }
        let mut pairs = vec![(key, value)];
        while self.eat_op(Op::Comma) {
            if self.at_op(Op::RBrace) {
                break;
            }
            let key = self.test()?;
            self.expect_op(Op::Colon)?;
            let value = self.test()?;
            pairs.push((key, value));
        }
        self.expect_op(Op::RBrace)?;
        Ok(Expr::Dict(pairs))
    }
}

fn bin(op: BinOp, left: Expr, right: Expr) -> Expr {
    Expr::Bin {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}

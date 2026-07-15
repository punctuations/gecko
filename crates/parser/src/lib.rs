mod parser;

pub use parser::{ParseError, parse};

#[cfg(test)]
mod tests {
    use super::parse;
    use ast::{BinOp, Expr, Param, Stmt};

    fn one(src: &str) -> Stmt {
        let m = parse(src).unwrap();
        assert_eq!(m.body.len(), 1, "expected one statement");
        m.body.into_iter().next().unwrap()
    }

    #[test]
    fn hello_world() {
        let s = one("print(\"hello world\")\n");
        let Stmt::Expr(Expr::Call {
            func,
            args,
            keywords,
        }) = s
        else {
            panic!("expected a call statement, got {s:?}");
        };
        assert_eq!(*func, Expr::Name("print".into()));
        assert_eq!(args, vec![Expr::Str("hello world".into())]);
        assert!(keywords.is_empty());
    }

    #[test]
    fn assignment() {
        let s = one("x = 1 + 2\n");
        let Stmt::Assign { targets, value } = s else {
            panic!("{s:?}")
        };
        assert_eq!(targets, vec![Expr::Name("x".into())]);
        let Expr::Bin { op, .. } = value else {
            panic!("{value:?}")
        };
        assert_eq!(op, BinOp::Add);
    }

    #[test]
    fn precedence_mul_over_add() {
        // 1 + 2 * 3 parses as 1 + (2 * 3).
        let s = one("1 + 2 * 3\n");
        let Stmt::Expr(Expr::Bin {
            op: BinOp::Add,
            right,
            ..
        }) = s
        else {
            panic!("{s:?}")
        };
        let Expr::Bin { op: BinOp::Mul, .. } = *right else {
            panic!("rhs not a product")
        };
    }

    #[test]
    fn power_is_right_associative() {
        // 2 ** 3 ** 2 parses as 2 ** (3 ** 2).
        let s = one("2 ** 3 ** 2\n");
        let Stmt::Expr(Expr::Bin {
            op: BinOp::Pow,
            right,
            ..
        }) = s
        else {
            panic!("{s:?}")
        };
        let Expr::Bin { op: BinOp::Pow, .. } = *right else {
            panic!("rhs not a power")
        };
    }

    #[test]
    fn funcdef_with_default() {
        let s = one("def f(a, b=1):\n    return a\n");
        let Stmt::FunctionDef { name, params, body } = s else {
            panic!("{s:?}")
        };
        assert_eq!(name, "f");
        assert_eq!(
            params[0],
            Param {
                name: "a".into(),
                default: None
            }
        );
        assert_eq!(params[1].name, "b");
        assert!(params[1].default.is_some());
        assert_eq!(body, vec![Stmt::Return(Some(Expr::Name("a".into())))]);
    }

    #[test]
    fn if_elif_else() {
        let s = one("if a:\n    pass\nelif b:\n    pass\nelse:\n    pass\n");
        let Stmt::If { orelse, .. } = s else {
            panic!("{s:?}")
        };
        // The elif becomes a nested If in the else branch.
        assert!(matches!(orelse.as_slice(), [Stmt::If { .. }]));
    }

    #[test]
    fn while_loop() {
        let s = one("while x < 10:\n    x = x + 1\n");
        assert!(matches!(s, Stmt::While { .. }));
    }

    #[test]
    fn for_loop_over_call() {
        let s = one("for i in range(3):\n    print(i)\n");
        let Stmt::For { target, iter, .. } = s else {
            panic!("{s:?}")
        };
        assert_eq!(target, Expr::Name("i".into()));
        assert!(matches!(iter, Expr::Call { .. }));
    }

    #[test]
    fn call_with_keyword() {
        let s = one("print(x, sep=\", \")\n");
        let Stmt::Expr(Expr::Call { args, keywords, .. }) = s else {
            panic!("{s:?}")
        };
        assert_eq!(args, vec![Expr::Name("x".into())]);
        assert_eq!(keywords.len(), 1);
        assert_eq!(keywords[0].arg, "sep");
    }

    #[test]
    fn comparison_chain() {
        let s = one("1 < x < 10\n");
        let Stmt::Expr(Expr::Compare {
            ops, comparators, ..
        }) = s
        else {
            panic!("{s:?}")
        };
        assert_eq!(ops.len(), 2);
        assert_eq!(comparators.len(), 2);
    }

    #[test]
    fn nonlocal_takes_names() {
        let s = one("nonlocal a, b\n");
        assert_eq!(s, Stmt::Nonlocal(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn nonlocal_needs_a_name() {
        assert!(parse("nonlocal\n").is_err());
        assert!(parse("nonlocal 1\n").is_err());
    }

    #[test]
    fn try_parses_handlers_else_finally() {
        let s = one(
            "try:\n    pass\nexcept ValueError as e:\n    pass\nexcept:\n    pass\nelse:\n    pass\nfinally:\n    pass\n",
        );
        let Stmt::Try {
            handlers,
            orelse,
            finalbody,
            ..
        } = s
        else {
            panic!("{s:?}")
        };
        assert_eq!(handlers.len(), 2);
        assert_eq!(handlers[0].name.as_deref(), Some("e"));
        assert!(handlers[1].typ.is_none());
        assert_eq!(orelse.len(), 1);
        assert_eq!(finalbody.len(), 1);
    }

    #[test]
    fn try_clause_order_is_enforced() {
        assert!(
            parse("try:\n    pass\nexcept:\n    pass\nexcept ValueError:\n    pass\n").is_err()
        );
        assert!(parse("try:\n    pass\nelse:\n    pass\n").is_err());
        assert!(parse("try:\n    pass\n").is_err());
    }

    #[test]
    fn errors_on_garbage() {
        assert!(parse("def (:\n").is_err());
    }
}

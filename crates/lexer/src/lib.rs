mod lexer;
mod token;

pub use lexer::{LexError, tokenize};
pub use token::{Keyword, Op, Span, StrPrefix, Token, TokenKind};

#[cfg(test)]
mod tests {
    use crate::{Keyword as Kw, Op as O, StrPrefix, TokenKind as K, tokenize};

    fn kinds(src: &str) -> Vec<K> {
        tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    fn int(digits: &str, radix: u32) -> K {
        K::Int {
            digits: digits.into(),
            radix,
        }
    }

    #[test]
    fn hello_world() {
        assert_eq!(
            kinds("print(\"hello world\")\n"),
            vec![
                K::Name("print".into()),
                K::Op(O::LParen),
                K::Str {
                    value: "hello world".into(),
                    prefix: StrPrefix::default()
                },
                K::Op(O::RParen),
                K::Newline,
                K::Eof,
            ]
        );
    }

    #[test]
    fn trailing_newline_is_synthesized() {
        assert_eq!(kinds("x"), vec![K::Name("x".into()), K::Newline, K::Eof]);
    }

    #[test]
    fn indentation_emits_indent_dedent() {
        assert_eq!(
            kinds("def f():\n    return 1\n"),
            vec![
                K::Keyword(Kw::Def),
                K::Name("f".into()),
                K::Op(O::LParen),
                K::Op(O::RParen),
                K::Op(O::Colon),
                K::Newline,
                K::Indent,
                K::Keyword(Kw::Return),
                int("1", 10),
                K::Newline,
                K::Dedent,
                K::Eof,
            ]
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            kinds("0xFF 0o17 0b101 42 2.75 1e3 .5 10_000"),
            vec![
                int("FF", 16),
                int("17", 8),
                int("101", 2),
                int("42", 10),
                K::Float(2.75),
                K::Float(1000.0),
                K::Float(0.5),
                int("10000", 10),
                K::Newline,
                K::Eof,
            ]
        );
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        assert_eq!(
            kinds("# lead\n\n   \nx = 1  # trailing\n"),
            vec![
                K::Name("x".into()),
                K::Op(O::Assign),
                int("1", 10),
                K::Newline,
                K::Eof
            ]
        );
    }

    #[test]
    fn brackets_join_lines_implicitly() {
        let k = kinds("f(\n    1,\n    2,\n)\n");
        assert!(!k.contains(&K::Indent), "no indent inside brackets");
        assert_eq!(k.iter().filter(|t| **t == K::Newline).count(), 1);
    }

    #[test]
    fn string_escapes_and_raw() {
        let k = kinds(r#""a\nb" r"a\nb""#);
        assert_eq!(
            k[0],
            K::Str {
                value: "a\nb".into(),
                prefix: StrPrefix::default()
            }
        );
        assert_eq!(
            k[1],
            K::Str {
                value: "a\\nb".into(),
                prefix: StrPrefix {
                    raw: true,
                    ..Default::default()
                }
            }
        );
    }

    #[test]
    fn operators() {
        let ops: Vec<O> = kinds("a **= b // c <= d := e")
            .iter()
            .filter_map(|t| if let K::Op(o) = t { Some(*o) } else { None })
            .collect();
        assert_eq!(ops, vec![O::DoubleStarEq, O::DoubleSlash, O::Le, O::Walrus]);
    }

    #[test]
    fn unterminated_string_errors() {
        assert!(tokenize("\"oops\n").is_err());
    }
}

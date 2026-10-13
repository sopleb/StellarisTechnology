use super::lexer::{Tok, Token, tokenize};
use super::{Block, Item, Pair, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: u32,
    pub message: String,
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

/// Parse a whole file. Always returns a block; problems are reported as errors
/// and the parser recovers by skipping the offending token.
pub fn parse(src: &str) -> (Block, Vec<ParseError>) {
    let mut p = Parser {
        toks: tokenize(src),
        pos: 0,
        errors: Vec::new(),
    };
    let block = p.block(true);
    (block, p.errors)
}

impl Parser {
    fn peek(&self, n: usize) -> Option<&Token> {
        self.toks.get(self.pos + n)
    }

    fn err(&mut self, line: u32, message: impl Into<String>) {
        self.errors.push(ParseError {
            line,
            message: message.into(),
        });
    }

    fn line(&self) -> u32 {
        self.peek(0).or(self.toks.last()).map_or(0, |t| t.line)
    }

    /// Parse items until `}` (nested) or EOF (top level).
    fn block(&mut self, top: bool) -> Block {
        let mut items = Vec::new();
        loop {
            let Some(tok) = self.peek(0).cloned() else {
                if !top {
                    self.err(self.line(), "unexpected end of file inside block");
                }
                break;
            };
            match tok.tok {
                Tok::Close => {
                    self.pos += 1;
                    if top {
                        self.err(tok.line, "unmatched '}'");
                        continue;
                    }
                    break;
                }
                Tok::Op(op) => {
                    self.pos += 1;
                    self.err(tok.line, format!("unexpected operator '{}'", op.as_str()));
                }
                Tok::Open => {
                    self.pos += 1;
                    items.push(Item::Bare(Value::Block(self.block(false))));
                }
                Tok::Scalar(_) | Tok::Str(_) => {
                    let key = match &tok.tok {
                        Tok::Scalar(s) | Tok::Str(s) => s.clone(),
                        _ => unreachable!(),
                    };
                    if let Some(Token { tok: Tok::Op(op), .. }) = self.peek(1).cloned() {
                        self.pos += 2;
                        match self.value(tok.line) {
                            Some(value) => items.push(Item::Pair(Pair {
                                key,
                                op,
                                value,
                                line: tok.line,
                            })),
                            None => self.err(tok.line, format!("missing value for '{key}'")),
                        }
                    } else {
                        self.pos += 1;
                        items.push(Item::Bare(match tok.tok {
                            Tok::Str(s) => Value::Str(s),
                            _ => Value::Scalar(key),
                        }));
                    }
                }
            }
        }
        Block { items }
    }

    fn value(&mut self, line: u32) -> Option<Value> {
        let tok = self.peek(0).cloned()?;
        match tok.tok {
            Tok::Open => {
                self.pos += 1;
                Some(Value::Block(self.block(false)))
            }
            Tok::Str(s) => {
                self.pos += 1;
                Some(Value::Str(s))
            }
            Tok::Scalar(s) => {
                self.pos += 1;
                // `hsv { ... }` — a scalar directly followed by a block is a tagged value.
                if matches!(self.peek(0), Some(Token { tok: Tok::Open, .. })) && is_tag(&s) {
                    self.pos += 1;
                    return Some(Value::Tagged(s, self.block(false)));
                }
                Some(Value::Scalar(s))
            }
            Tok::Close | Tok::Op(_) => {
                let _ = line;
                None
            }
        }
    }
}

fn is_tag(s: &str) -> bool {
    matches!(s, "hsv" | "rgb" | "hsv360" | "hex" | "LIST")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::Op;
    use pretty_assertions::assert_eq;

    fn ok(src: &str) -> Block {
        let (b, e) = parse(src);
        assert_eq!(e, vec![], "unexpected errors");
        b
    }

    #[test]
    fn nested_pairs_and_lists() {
        let b = ok(r#"
            @tier1cost = 120
            tech_a = {
                cost = @tier1cost
                category = { voidcraft }
                prerequisites = { "tech_b" tech_c }
                start_tech = yes
                potential = { years_passed >= 10 NOT = { has_ethic = ethic_x } }
            }
        "#);
        assert_eq!(b.get_str("@tier1cost"), Some("120"));
        let t = b.get_block("tech_a").unwrap();
        assert_eq!(t.get_str("cost"), Some("@tier1cost"));
        assert_eq!(t.get_block("category").unwrap().bare_strings(), vec!["voidcraft"]);
        assert_eq!(
            t.get_block("prerequisites").unwrap().bare_strings(),
            vec!["tech_b", "tech_c"]
        );
        assert_eq!(t.get_bool("start_tech"), Some(true));
        let pot = t.get_block("potential").unwrap();
        let yp = pot.pairs().next().unwrap();
        assert_eq!((yp.key.as_str(), yp.op), ("years_passed", Op::Ge));
    }

    #[test]
    fn tagged_and_math_and_macros() {
        let b = ok("c = hsv { 0.1 0.2 0.3 } m = @[ x + 10 ] a = $BONUS|0$ q ?= yes");
        assert!(matches!(b.get("c"), Some(Value::Tagged(t, _)) if t == "hsv"));
        assert_eq!(b.get_str("m"), Some("@[ x + 10 ]"));
        assert_eq!(b.get_str("a"), Some("$BONUS|0$"));
        assert_eq!(b.pairs().last().unwrap().op, Op::QEq);
    }

    #[test]
    fn numeric_keys_and_bare_blocks() {
        let b = ok("0 = { } 1 = { previously_unlocked = 8 } x = { { a } { b } }");
        assert_eq!(b.get_block("1").unwrap().get_str("previously_unlocked"), Some("8"));
        assert_eq!(b.get_block("x").unwrap().bare().count(), 2);
    }

    #[test]
    fn recovers_from_errors() {
        let (b, e) = parse("a = { b = 1 } } c = 2 d = = 3 e = { f = 1");
        assert_eq!(b.get_str("c"), Some("2"));
        assert_eq!(b.get_block("e").unwrap().get_str("f"), Some("1"));
        assert!(e.len() >= 2, "{e:?}");
    }

    #[test]
    fn later_key_wins() {
        let b = ok("a = 1 a = 2");
        assert_eq!(b.get_str("a"), Some("2"));
        assert_eq!(b.get_all("a").count(), 2);
    }

    #[test]
    fn decode_latin1_and_bom() {
        assert_eq!(crate::script::decode(b"\xEF\xBB\xBFa = 1"), "a = 1");
        assert_eq!(crate::script::decode(b"n = \"Caf\xe9\""), "n = \"Café\"");
    }
}

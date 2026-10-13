use super::Op;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Open,
    Close,
    Op(Op),
    Scalar(String),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
}

fn is_delim(c: char) -> bool {
    c.is_whitespace() || matches!(c, '{' | '}' | '=' | '<' | '>' | '!' | '?' | '"' | '#')
}

/// Tokenize Paradox script. Never fails: unknown characters become part of scalars.
pub fn tokenize(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1u32;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '{' => {
                out.push(Token { tok: Tok::Open, line });
                i += 1;
            }
            '}' => {
                out.push(Token { tok: Tok::Close, line });
                i += 1;
            }
            '=' | '<' | '>' | '!' | '?' => {
                let next = chars.get(i + 1).copied();
                let (op, len) = match (c, next) {
                    ('=', Some('=')) => (Some(Op::EqEq), 2),
                    ('=', _) => (Some(Op::Eq), 1),
                    ('<', Some('=')) => (Some(Op::Le), 2),
                    ('<', Some('>')) => (Some(Op::Ne), 2),
                    ('<', _) => (Some(Op::Lt), 1),
                    ('>', Some('=')) => (Some(Op::Ge), 2),
                    ('>', _) => (Some(Op::Gt), 1),
                    ('!', Some('=')) => (Some(Op::Ne), 2),
                    ('?', Some('=')) => (Some(Op::QEq), 2),
                    _ => (None, 1),
                };
                match op {
                    Some(op) => out.push(Token { tok: Tok::Op(op), line }),
                    // Lone `!` or `?`: keep it as a scalar so nothing is silently lost.
                    None => out.push(Token { tok: Tok::Scalar(c.to_string()), line }),
                }
                i += len;
            }
            '"' => {
                let start_line = line;
                let mut s = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && chars.get(i + 1) == Some(&'"') {
                        s.push('"');
                        i += 2;
                        continue;
                    }
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    s.push(chars[i]);
                    i += 1;
                }
                i += 1; // closing quote (or EOF)
                out.push(Token { tok: Tok::Str(s), line: start_line });
            }
            '@' if chars.get(i + 1) == Some(&'[') => {
                // Inline math `@[ ... ]` is a single scalar.
                let mut s = String::new();
                while i < chars.len() {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    s.push(chars[i]);
                    i += 1;
                    if s.ends_with(']') {
                        break;
                    }
                }
                out.push(Token { tok: Tok::Scalar(s), line });
            }
            _ => {
                let mut s = String::new();
                while i < chars.len() && !is_delim(chars[i]) {
                    // Embedded inline math (`case_@[ x + 1 ]`) is part of the scalar.
                    if chars[i] == '@' && chars.get(i + 1) == Some(&'[') {
                        while i < chars.len() {
                            if chars[i] == '\n' {
                                line += 1;
                            }
                            s.push(chars[i]);
                            i += 1;
                            if s.ends_with(']') {
                                break;
                            }
                        }
                        continue;
                    }
                    s.push(chars[i]);
                    i += 1;
                }
                out.push(Token { tok: Tok::Scalar(s), line });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        tokenize(s).into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn operators() {
        assert_eq!(
            toks("a = b c == d e != f g <> h i < j k > l m <= n o >= p q ?= r")
                .into_iter()
                .filter_map(|t| match t {
                    Tok::Op(o) => Some(o),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![Op::Eq, Op::EqEq, Op::Ne, Op::Ne, Op::Lt, Op::Gt, Op::Le, Op::Ge, Op::QEq]
        );
    }

    #[test]
    fn strings_comments_math() {
        assert_eq!(
            toks("k = \"a \\\"b\\\" # c\" # comment\nx = @[ a + 10 ] y = value:f|A|$B$|"),
            vec![
                Tok::Scalar("k".into()),
                Tok::Op(Op::Eq),
                Tok::Str("a \"b\" # c".into()),
                Tok::Scalar("x".into()),
                Tok::Op(Op::Eq),
                Tok::Scalar("@[ a + 10 ]".into()),
                Tok::Scalar("y".into()),
                Tok::Op(Op::Eq),
                Tok::Scalar("value:f|A|$B$|".into()),
            ]
        );
    }

    #[test]
    fn embedded_math() {
        assert_eq!(toks("s = a_@[ 1 - (x) ] b"), vec![
            Tok::Scalar("s".into()),
            Tok::Op(Op::Eq),
            Tok::Scalar("a_@[ 1 - (x) ]".into()),
            Tok::Scalar("b".into()),
        ]);
    }

    #[test]
    fn line_numbers() {
        let t = tokenize("a\n\"multi\nline\"\nb");
        assert_eq!(t.iter().map(|t| t.line).collect::<Vec<_>>(), vec![1, 2, 4]);
    }
}

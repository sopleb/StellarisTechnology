//! `@variable` resolution and best-effort `@[ inline math ]` evaluation.

use crate::script::{Block, Value};
use std::collections::HashMap;

/// Variables visible to a file: globals (scripted_variables) overlaid by the file's own.
#[derive(Debug, Default, Clone)]
pub struct Vars {
    map: HashMap<String, String>,
}

impl Vars {
    /// Collect top-level `@name = value` definitions, keyed without the `@`.
    pub fn add_from(&mut self, block: &Block) {
        for p in block.pairs() {
            if let (Some(name), Some(v)) = (p.key.strip_prefix('@'), p.value.as_str()) {
                self.map.insert(name.to_string(), v.to_string());
            }
        }
    }

    /// A copy with `block`'s own variables layered on top.
    pub fn with_file(&self, block: &Block) -> Vars {
        let mut v = self.clone();
        v.add_from(block);
        v
    }

    /// Resolve a scalar (`12`, `@var`, `@[ a * 2 ]`) to a number.
    pub fn number(&self, s: &str) -> Option<f64> {
        self.number_depth(s, 0)
    }

    /// Replace every `@[ expr ]` in `s` with its value (integers without a decimal point).
    pub fn eval_inline_math(&self, s: &str) -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(start) = rest.find("@[") {
            let Some(len) = rest[start..].find(']') else { break };
            out.push_str(&rest[..start]);
            let expr = &rest[start..start + len + 1];
            match self.number(expr) {
                Some(n) if n.fract() == 0.0 => out.push_str(&format!("{}", n as i64)),
                Some(n) => out.push_str(&n.to_string()),
                None => out.push_str(expr),
            }
            rest = &rest[start + len + 1..];
        }
        out.push_str(rest);
        out
    }

    pub fn number_of(&self, v: &Value) -> Option<f64> {
        v.as_str().and_then(|s| self.number(s))
    }

    fn number_depth(&self, s: &str, depth: u8) -> Option<f64> {
        if depth > 8 {
            return None;
        }
        let s = s.trim();
        if let Some(expr) = s.strip_prefix("@[").and_then(|e| e.strip_suffix(']')) {
            return eval(expr, &|name| self.number_depth(name, depth + 1));
        }
        if let Some(name) = s.strip_prefix('@') {
            return self.map.get(name).and_then(|v| self.number_depth(v, depth + 1));
        }
        if let Ok(n) = s.trim_end_matches('%').parse::<f64>() {
            return Some(n);
        }
        // Bare identifiers inside math refer to variables without the `@`.
        self.map.get(s).and_then(|v| self.number_depth(v, depth + 1))
    }
}

/// Evaluate `+ - * /` arithmetic with parentheses; identifiers go through `lookup`.
fn eval(expr: &str, lookup: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    let toks = lex(expr)?;
    let mut pos = 0;
    let v = sum(&toks, &mut pos, lookup)?;
    (pos == toks.len()).then_some(v)
}

#[derive(Debug, Clone, PartialEq)]
enum T {
    Num(f64),
    Ident(String),
    Sym(char),
}

fn lex(s: &str) -> Option<Vec<T>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ch if ch.is_whitespace() => i += 1,
            '+' | '-' | '*' | '/' | '%' | '(' | ')' => {
                out.push(T::Sym(c[i]));
                i += 1;
            }
            ch if ch.is_ascii_digit() || ch == '.' => {
                let st = i;
                while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                    i += 1;
                }
                out.push(T::Num(c[st..i].iter().collect::<String>().parse().ok()?));
            }
            ch if ch.is_alphanumeric() || ch == '_' || ch == '@' => {
                let st = i;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '@') {
                    i += 1;
                }
                out.push(T::Ident(c[st..i].iter().collect()));
            }
            _ => return None,
        }
    }
    Some(out)
}

fn sum(t: &[T], p: &mut usize, l: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    let mut v = product(t, p, l)?;
    while let Some(T::Sym(op @ ('+' | '-'))) = t.get(*p) {
        *p += 1;
        let r = product(t, p, l)?;
        v = if *op == '+' { v + r } else { v - r };
    }
    Some(v)
}

fn product(t: &[T], p: &mut usize, l: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    let mut v = unary(t, p, l)?;
    while let Some(T::Sym(op @ ('*' | '/' | '%'))) = t.get(*p) {
        *p += 1;
        let r = unary(t, p, l)?;
        v = match op {
            '*' => v * r,
            '/' => v / r,
            _ => v % r,
        };
    }
    Some(v)
}

fn unary(t: &[T], p: &mut usize, l: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    match t.get(*p)? {
        T::Sym('-') => {
            *p += 1;
            Some(-unary(t, p, l)?)
        }
        T::Sym('(') => {
            *p += 1;
            let v = sum(t, p, l)?;
            (t.get(*p) == Some(&T::Sym(')'))).then(|| *p += 1)?;
            Some(v)
        }
        T::Num(n) => {
            *p += 1;
            Some(*n)
        }
        T::Ident(name) => {
            *p += 1;
            l(name)
        }
        T::Sym(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::parse;

    #[test]
    fn inline_math_in_strings() {
        let v = Vars::default();
        assert_eq!(v.eval_inline_math("case_@[ 1 + 1 ]_x"), "case_2_x");
        assert_eq!(v.eval_inline_math("a@[ 1 / 2 ]"), "a0.5");
        assert_eq!(v.eval_inline_math("no math"), "no math");
    }

    #[test]
    fn resolves_vars_and_math() {
        let (g, _) = parse("@base = 100 @mult = @base");
        let mut v = Vars::default();
        v.add_from(&g);
        let (f, _) = parse("@local = 3");
        let v = v.with_file(&f);
        assert_eq!(v.number("42"), Some(42.0));
        assert_eq!(v.number("@mult"), Some(100.0));
        assert_eq!(v.number("@[ base * local + 10 ]"), Some(310.0));
        assert_eq!(v.number("@[ (base - 40) / -2 ]"), Some(-30.0));
        assert_eq!(v.number("@[-local]"), Some(-3.0));
        assert_eq!(v.number("@missing"), None);
        assert_eq!(v.number("@[ ((-0.5 % 1) + 1) % 1 ]"), Some(0.5));
        assert_eq!(v.number("@[ 1 + ]"), None);
    }
}

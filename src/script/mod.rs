//! Paradox script (Clausewitz) parsing: AST, lexer and an error-tolerant parser.

mod lexer;
mod parser;

pub use parser::{ParseError, parse};

use std::path::Path;

/// Comparison/assignment operator between a key and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    EqEq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    QEq,
}

impl Op {
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Eq => "=",
            Op::EqEq => "==",
            Op::Ne => "!=",
            Op::Lt => "<",
            Op::Gt => ">",
            Op::Le => "<=",
            Op::Ge => ">=",
            Op::QEq => "?=",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Unquoted token: numbers, booleans, identifiers, `@vars`, `@[ math ]`, `value:x|A|B|`.
    Scalar(String),
    /// Quoted string, without the quotes.
    Str(String),
    Block(Block),
    /// `hsv { 0.1 0.2 0.3 }` style tagged blocks.
    Tagged(String, Block),
}

impl Value {
    /// Text of a scalar or quoted string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Scalar(s) | Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_block(&self) -> Option<&Block> {
        match self {
            Value::Block(b) | Value::Tagged(_, b) => Some(b),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.as_str()? {
            "yes" | "true" => Some(true),
            "no" | "false" => Some(false),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub key: String,
    pub op: Op,
    pub value: Value,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Pair(Pair),
    Bare(Value),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub items: Vec<Item>,
}

impl Block {
    pub fn pairs(&self) -> impl Iterator<Item = &Pair> {
        self.items.iter().filter_map(|i| match i {
            Item::Pair(p) => Some(p),
            Item::Bare(_) => None,
        })
    }

    pub fn bare(&self) -> impl Iterator<Item = &Value> {
        self.items.iter().filter_map(|i| match i {
            Item::Bare(v) => Some(v),
            Item::Pair(_) => None,
        })
    }

    /// Last value for `key` (later definitions win, as in the game).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.pairs().filter(|p| p.key == key).last().map(|p| &p.value)
    }

    pub fn get_all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Pair> + 'a {
        self.pairs().filter(move |p| p.key == key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn get_block(&self, key: &str) -> Option<&Block> {
        self.get(key).and_then(Value::as_block)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    /// Bare scalar/string values of a list block such as `{ a "b" c }`.
    pub fn bare_strings(&self) -> Vec<String> {
        self.bare().filter_map(|v| v.as_str().map(str::to_string)).collect()
    }
}

/// Decode a game file: strip a UTF-8 BOM, fall back to Latin-1 for invalid UTF-8.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Read and parse a file, logging (not failing on) syntax errors.
pub fn parse_file(path: &Path) -> std::io::Result<Block> {
    let text = decode(&std::fs::read(path)?);
    let (block, errors) = parse(&text);
    for e in errors {
        log::debug!("{}:{}: {}", path.display(), e.line, e.message);
    }
    Ok(block)
}

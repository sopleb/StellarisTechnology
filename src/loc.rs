//! Paradox localisation (`*_l_<lang>.yml`) loading and text resolution.

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$([A-Za-z0-9_.\-]+)(\|[^$]*)?\$").unwrap());
static COLOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"§[A-Za-z0-9!]").unwrap());
static ICON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"£[^£\s]+£\s?").unwrap());

/// Case-insensitive key → raw text table. Later inserts override earlier ones.
#[derive(Debug, Default, Clone)]
pub struct Localisation {
    map: HashMap<String, String>,
}

impl Localisation {
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn insert(&mut self, key: &str, value: &str) {
        self.map.insert(key.to_lowercase(), value.to_string());
    }

    /// Parse one `.yml` file's text and merge it in.
    pub fn load_str(&mut self, text: &str) {
        for (k, v) in parse_yml(text) {
            self.map.insert(k.to_lowercase(), v);
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(&key.to_lowercase())
    }

    pub fn raw(&self, key: &str) -> Option<&str> {
        self.map.get(&key.to_lowercase()).map(String::as_str)
    }

    /// Resolved, display-ready text for `key`, if it exists.
    pub fn get(&self, key: &str) -> Option<String> {
        self.raw(key).map(|raw| self.render(raw))
    }

    /// Resolved text for `key`, or the key itself.
    pub fn name(&self, key: &str) -> String {
        self.get(key).unwrap_or_else(|| key.to_string())
    }

    /// First key among `keys` that exists, resolved.
    pub fn first(&self, keys: &[String]) -> Option<String> {
        keys.iter().find_map(|k| self.get(k))
    }

    /// Resolve `$ref$` references and strip formatting codes.
    pub fn render(&self, raw: &str) -> String {
        let mut s = raw.replace("\\n", "\n");
        for _ in 0..6 {
            if !s.contains('$') {
                break;
            }
            let next = REF
                .replace_all(&s, |c: &regex::Captures| match self.raw(&c[1]) {
                    Some(v) => v.replace("\\n", "\n"),
                    None => c[0].to_string(),
                })
                .into_owned();
            if next == s {
                break;
            }
            s = next;
        }
        let s = COLOR.replace_all(&s, "");
        let s = ICON.replace_all(&s, "");
        s.trim().to_string()
    }
}

/// Parse Paradox's YAML-like format: `l_english:` header, then ` key:0 "text"` lines.
pub fn parse_yml(text: &str) -> Vec<(String, String)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(colon) = line.find(':') else { continue };
        let key = line[..colon].trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            continue;
        }
        let rest = &line[colon + 1..];
        let Some(start) = rest.find('"') else { continue };
        let Some(end) = closing_quote(rest, start) else {
            continue;
        };
        if end <= start {
            continue;
        }
        // Only a version number may sit between the colon and the opening quote.
        if !rest[..start].trim().chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        out.push((key.to_string(), rest[start + 1..end].to_string()));
    }
    out
}

/// The closing quote: the first quote after `start` that is followed only by
/// whitespace or a `#` comment (so quotes inside the text or a comment are kept/ignored).
fn closing_quote(rest: &str, start: usize) -> Option<usize> {
    rest.char_indices()
        .skip_while(|(i, _)| *i <= start)
        .filter(|(_, c)| *c == '"')
        .map(|(i, _)| i)
        .find(|&i| {
            let after = rest[i + 1..].trim_start();
            after.is_empty() || after.starts_with('#')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yml_lines() {
        let v =
            parse_yml("\u{feff}l_english:\n # c\n a:0 \"Hello \"x\" there\" # tail\n b: \"B\"\n bad line\n c:1 \"\"\n");
        assert_eq!(
            v,
            vec![
                ("a".into(), "Hello \"x\" there".into()),
                ("b".into(), "B".into()),
                ("c".into(), "".into())
            ]
        );
    }

    #[test]
    fn quoted_comment_after_value() {
        assert_eq!(parse_yml(" k:0 \"text\" # \"note\""), vec![("k".into(), "text".into())]);
    }

    #[test]
    fn resolves_and_cleans() {
        let mut l = Localisation::default();
        l.load_str(
            "l_english:\n corvette:0 \"$MOD_HULL$: §G+10%§!\\n£energy£ $unknown$ $x|y$\"\n mod_hull:0 \"Hull\"\n",
        );
        assert_eq!(l.get("CORVETTE").unwrap(), "Hull: +10%\n$unknown$ $x|y$");
        assert_eq!(l.name("missing_key"), "missing_key");
    }

    #[test]
    fn later_wins() {
        let mut l = Localisation::default();
        l.load_str(" a:0 \"one\"");
        l.load_str(" a:0 \"two\"");
        assert_eq!(l.get("a").unwrap(), "two");
    }
}

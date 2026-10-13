//! Paradox localisation (`*_l_<lang>.yml`) loading and text resolution.

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$([A-Za-z0-9_.\-]+)(\|[^$]*)?\$").unwrap());
static COLOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"§[A-Za-z0-9!]").unwrap());
/// 4.x concept links: `['civic:civic_x']` or `['civic:$CIVIC$', $CIVIC_NAME$]` (link target, shown text).
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\['([^'\]]*)'\s*(?:,\s*([^\]]*))?\]").unwrap());
/// Scripted localisation calls such as `[GetFarmerSwapPluralWithIcon]`, and calls on a game
/// object such as `[farmer.GetNamePlural]`.
static FUNC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([A-Za-z_][A-Za-z0-9_]*)(?:\.([A-Za-z]+))?\]\s?").unwrap());
static ICON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"£[^£\s]+£\s?").unwrap());

/// Key → raw text table. Paradox keys are case-sensitive (`astral_threads` and
/// `ASTRAL_THREADS` can differ), so an exact match wins and a case-insensitive one
/// is the fallback. Later inserts override earlier ones.
#[derive(Debug, Default, Clone)]
pub struct Localisation {
    exact: HashMap<String, String>,
    map: HashMap<String, String>,
    /// `common/scripted_loc` function name → the localisation key it shows by default.
    functions: HashMap<String, String>,
}

impl Localisation {
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn insert(&mut self, key: &str, value: &str) {
        self.exact.insert(key.to_string(), value.to_string());
        self.map.insert(key.to_lowercase(), value.to_string());
    }

    /// Parse one `.yml` file's text and merge it in.
    pub fn load_str(&mut self, text: &str) {
        for (k, v) in parse_yml(text) {
            self.insert(&k, &v);
        }
    }

    /// Register a scripted localisation function; the first definition wins, as in the game.
    pub fn add_function(&mut self, name: &str, default_key: &str) {
        self.functions
            .entry(name.to_string())
            .or_insert_with(|| default_key.to_string());
    }

    pub fn contains(&self, key: &str) -> bool {
        self.raw(key).is_some()
    }

    pub fn raw(&self, key: &str) -> Option<&str> {
        self.exact
            .get(key)
            .or_else(|| self.map.get(&key.to_lowercase()))
            .map(String::as_str)
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

    /// Resolve `$ref$` references and concept links, and strip formatting codes.
    pub fn render(&self, raw: &str) -> String {
        let s = self.resolve_refs(raw);
        // A link shows its second argument; without one, the game shows the linked item's name.
        let s = LINK.replace_all(&s, |c: &regex::Captures| match c.get(2).map(|m| m.as_str().trim()) {
            Some(shown) if !shown.is_empty() => shown.to_string(),
            _ => {
                let target = c[1].rsplit(':').next().unwrap_or("");
                self.raw(target)
                    .map(|t| self.resolve_refs(t))
                    .unwrap_or_else(|| target.to_string())
            }
        });
        // Scripted localisation depends on the viewer's empire; show its default text.
        let s = FUNC.replace_all(&s, |c: &regex::Captures| {
            self.call(&c[1], c.get(2).map(|m| m.as_str()), &c[0])
        });
        let s = FUNC.replace_all(&s, |c: &regex::Captures| {
            self.call(&c[1], c.get(2).map(|m| m.as_str()), &c[0])
        });
        let s = COLOR.replace_all(&s, "");
        let s = ICON.replace_all(&s, "");
        s.trim().to_string()
    }

    /// Text for `[name]` or `[object.Method]`; `original` when there's no sensible text.
    fn call(&self, name: &str, method: Option<&str>, original: &str) -> String {
        let space = if original.ends_with(char::is_whitespace) {
            " "
        } else {
            ""
        };
        let text = match method {
            None => self
                .functions
                .get(name)
                .and_then(|k| self.raw(k))
                .map(|t| self.resolve_refs(t)),
            // Icons have no text form.
            Some("GetIcon") => return String::new(),
            Some("GetName") => [name.to_string(), format!("job_{name}")]
                .iter()
                .find_map(|k| self.raw(k))
                .map(|t| self.resolve_refs(t)),
            Some("GetNamePlural") => [format!("{name}_plural"), format!("job_{name}_plural")]
                .iter()
                .find_map(|k| self.raw(k))
                .map(|t| self.resolve_refs(t)),
            Some(_) => None,
        };
        match text {
            Some(t) => format!("{t}{space}"),
            None => original.to_string(),
        }
    }

    fn resolve_refs(&self, raw: &str) -> String {
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
        s
    }
}

/// Remove `$PARAM$` placeholders that were left unresolved (filled in by the game at runtime).
pub fn strip_refs(text: &str) -> String {
    let s = REF.replace_all(text, "");
    let mut out = String::with_capacity(s.len());
    for w in s.split(' ').filter(|w| !w.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
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
    fn case_sensitive_keys_win_over_case_insensitive() {
        let mut l = Localisation::default();
        l.load_str(" astral_threads: \"Astral Threads\"\n ASTRAL_THREADS: \"$astral_threads$\"\n MOD_X: \"X\"");
        assert_eq!(l.get("ASTRAL_THREADS").unwrap(), "Astral Threads");
        assert_eq!(l.get("astral_threads").unwrap(), "Astral Threads");
        assert_eq!(l.get("mod_x").unwrap(), "X");
    }

    #[test]
    fn concept_links_show_their_text() {
        let mut l = Localisation::default();
        l.load_str(
            " civic_x: \"Elite\"\n t: \"Has ['civic:$CIVIC$', $CIVIC_NAME$] Civic\"\n u: \"See ['civic:civic_x'] and ['concept_y']\"",
        );
        assert_eq!(strip_refs(&l.get("t").unwrap()), "Has Civic");
        assert_eq!(l.get("u").unwrap(), "See Elite and concept_y");
    }

    #[test]
    fn scripted_loc_shows_default() {
        let mut l = Localisation::default();
        l.load_str(" farmers: \"£job_farmer£ Farmers\"\n m: \"Produced from [GetFarmers]: +5% [Unknown]\"");
        l.add_function("GetFarmers", "farmers");
        l.add_function("GetFarmers", "other");
        assert_eq!(l.get("m").unwrap(), "Produced from Farmers: +5% [Unknown]");
        l.load_str(
            " job_miner_plural: \"Miners\"\n n: \"From [miner.GetIcon] [miner.GetNamePlural]: +5% [Root.GetName]\"",
        );
        assert_eq!(l.get("n").unwrap(), "From Miners: +5% [Root.GetName]");
    }

    #[test]
    fn strips_unresolved_params() {
        assert_eq!(strip_refs("Has $CIVIC|Y$ Civic"), "Has Civic");
    }

    #[test]
    fn later_wins() {
        let mut l = Localisation::default();
        l.load_str(" a:0 \"one\"");
        l.load_str(" a:0 \"two\"");
        assert_eq!(l.get("a").unwrap(), "two");
    }
}

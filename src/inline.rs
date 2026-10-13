//! Expansion of `inline_script = ...` references from `common/inline_scripts`.

use crate::script::{self, Block, Item, Value};
use crate::source::GameSource;
use crate::vars::Vars;
use regex::Regex;
use std::sync::LazyLock;

static PARAM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$([A-Za-z0-9_]+)(?:\|([^$]*))?\$").unwrap());

/// Replace every `inline_script` item in `block` (recursively) with the script's contents.
pub fn expand(block: &Block, src: &GameSource, vars: &Vars) -> Block {
    expand_depth(block, src, vars, 0)
}

fn expand_depth(block: &Block, src: &GameSource, vars: &Vars, depth: u8) -> Block {
    let mut items = Vec::with_capacity(block.items.len());
    for item in &block.items {
        match item {
            Item::Pair(p) if p.key == "inline_script" && depth < 8 => match load(&p.value, src, vars) {
                Some(inner) => items.extend(expand_depth(&inner, src, vars, depth + 1).items),
                None => {
                    let what = match &p.value {
                        Value::Block(b) => b.get_str("script").unwrap_or("?").to_string(),
                        v => v.as_str().unwrap_or("?").to_string(),
                    };
                    log::warn!("could not expand inline_script {what} (line {})", p.line);
                    items.push(item.clone());
                }
            },
            Item::Pair(p) => {
                let mut p = p.clone();
                if let Value::Block(b) = &p.value {
                    p.value = Value::Block(expand_depth(b, src, vars, depth));
                }
                items.push(Item::Pair(p));
            }
            Item::Bare(Value::Block(b)) => items.push(Item::Bare(Value::Block(expand_depth(b, src, vars, depth)))),
            Item::Bare(_) => items.push(item.clone()),
        }
    }
    Block { items }
}

fn load(v: &Value, src: &GameSource, vars: &Vars) -> Option<Block> {
    let (path, params): (String, Vec<(String, String)>) = match v {
        Value::Scalar(s) | Value::Str(s) => (s.clone(), vec![]),
        Value::Block(b) => (
            b.get_str("script")?.to_string(),
            b.pairs()
                .filter(|p| p.key != "script")
                .filter_map(|p| p.value.as_str().map(|v| (p.key.clone(), v.to_string())))
                .collect(),
        ),
        Value::Tagged(..) => return None,
    };
    // Script names may be computed, e.g. `case_@[ toggle * 1 ]`.
    let path = vars.eval_inline_math(&path);
    let file = src.find(&format!("common/inline_scripts/{path}.txt"))?;
    let text = script::decode(&std::fs::read(&file.path).ok()?);
    let text = substitute(&text, &params);
    Some(script::parse(&text).0)
}

/// Substitute `$NAME$` / `$NAME|default$`; unknown names fall back to their default.
pub fn substitute(text: &str, params: &[(String, String)]) -> String {
    PARAM
        .replace_all(text, |c: &regex::Captures| {
            match params.iter().find(|(k, _)| k == &c[1]) {
                Some((_, v)) => v.clone(),
                None => c.get(2).map_or_else(|| c[0].to_string(), |d| d.as_str().to_string()),
            }
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_params_and_defaults() {
        let p = vec![("TECH".to_string(), "tech_a".to_string())];
        assert_eq!(
            substitute("x = $TECH$ y = $N|3$ z = $Q$", &p),
            "x = tech_a y = 3 z = $Q$"
        );
    }

    #[test]
    fn expands_from_source() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("common/inline_scripts/technologies");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("w.txt"),
            "modifier = { factor = 2 has_technology = $TECHNOLOGY$ }",
        )
        .unwrap();
        let src = GameSource::new(t.path().to_path_buf());
        let (b, _) = script::parse(
            "weight_modifier = { factor = 1 inline_script = { script = technologies/w TECHNOLOGY = tech_x } }",
        );
        let e = expand(&b, &src, &Vars::default());
        let wm = e.get_block("weight_modifier").unwrap();
        assert_eq!(
            wm.get_block("modifier").unwrap().get_str("has_technology"),
            Some("tech_x")
        );
        assert!(wm.get("inline_script").is_none());
    }
}

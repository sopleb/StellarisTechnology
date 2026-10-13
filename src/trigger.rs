//! Converting trigger blocks into [`Trigger`] trees with generic, localisation-driven text.

use crate::loc::Localisation;
use crate::model::Trigger;
use crate::script::{Block, Item, Op, Value};
use std::collections::HashMap;

pub struct TriggerCtx<'a> {
    pub loc: &'a Localisation,
    pub scripted: &'a HashMap<String, Block>,
}

impl TriggerCtx<'_> {
    /// Convert a whole block; several conditions are wrapped in an implicit AND.
    pub fn block(&self, b: &Block) -> Option<Trigger> {
        let mut nodes = self.items(b, 0);
        match nodes.len() {
            0 => None,
            1 => nodes.pop(),
            _ => Some(Trigger {
                key: "AND".into(),
                op: "=".into(),
                value: None,
                text: "All of the following".into(),
                children: nodes,
                scripted: None,
            }),
        }
    }

    fn items(&self, b: &Block, depth: u8) -> Vec<Trigger> {
        b.items
            .iter()
            .map(|i| match i {
                Item::Pair(p) => self.node(&p.key, p.op, &p.value, depth),
                Item::Bare(v) => self.node("", Op::Eq, v, depth),
            })
            .collect()
    }

    fn node(&self, key: &str, op: Op, v: &Value, depth: u8) -> Trigger {
        let mut t = Trigger { key: key.into(), op: op.as_str().into(), value: None, children: vec![], text: String::new(), scripted: None };
        match v {
            Value::Block(b) | Value::Tagged(_, b) => {
                t.children = self.items(b, depth);
                t.text = self.block_text(key, &t.children);
            }
            Value::Scalar(s) | Value::Str(s) => {
                t.value = Some(s.clone());
                t.text = self.leaf_text(key, op, s);
                if depth < 2
                    && let (Some(body), Some(_)) = (self.scripted.get(key), v.as_bool())
                {
                    t.scripted = Some(self.items(body, depth + 1));
                }
            }
        }
        t
    }

    fn block_text(&self, key: &str, children: &[Trigger]) -> String {
        match key {
            "AND" => "All of the following".into(),
            "OR" => "At least one of the following".into(),
            "NOR" => "None of the following".into(),
            "NAND" => "At least one of the following must be false".into(),
            "NOT" if children.len() == 1 => format!("NOT: {}", children[0].text),
            "NOT" => "None of the following".into(),
            "" => "Group".into(),
            _ => format!("{}:", self.key_name(key)),
        }
    }

    fn leaf_text(&self, key: &str, op: Op, value: &str) -> String {
        if key.is_empty() {
            return self.value_name(value);
        }
        if key == "always" {
            return if value == "yes" { "Always".into() } else { "Never".into() };
        }
        let name = self.key_name(key);
        match (op, value) {
            (Op::Eq, "yes") => name,
            (Op::Eq, "no") => format!("NOT: {name}"),
            (Op::Eq, _) => format!("{name}: {}", self.value_name(value)),
            _ => format!("{name} {} {}", op.as_str(), self.value_name(value)),
        }
    }

    /// Localised name of a trigger key, else a humanized form of it.
    fn key_name(&self, key: &str) -> String {
        self.loc.get(&format!("trigger_{key}")).unwrap_or_else(|| humanize(key))
    }

    fn value_name(&self, value: &str) -> String {
        if value.parse::<f64>().is_ok() {
            return value.into();
        }
        self.loc.get(value).filter(|s| !s.is_empty() && !s.contains('\n')).unwrap_or_else(|| value.into())
    }
}

/// `has_ascension_perk` → `Has ascension perk`.
pub fn humanize(key: &str) -> String {
    let s = key.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => s,
    }
}

/// Flatten a trigger tree into indented lines, for plain-text display.
pub fn to_lines(t: &Trigger) -> String {
    fn walk(t: &Trigger, depth: usize, out: &mut Vec<String>) {
        out.push(format!("{}{}", "  ".repeat(depth), t.text));
        for c in &t.children {
            walk(c, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(t, 0, &mut out);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::parse;

    #[test]
    fn renders_generic_tree() {
        let mut loc = Localisation::default();
        loc.load_str(" tech_lasers_1:0 \"Red Lasers\"\n ethic_pacifist:0 \"Pacifist\"");
        let (s, _) = parse("is_cool = { has_ethic = ethic_pacifist }");
        let scripted: HashMap<String, Block> = s.pairs().map(|p| (p.key.clone(), p.value.as_block().unwrap().clone())).collect();
        let ctx = TriggerCtx { loc: &loc, scripted: &scripted };
        let (b, _) = parse(
            "has_technology = tech_lasers_1 years_passed > 20 is_gestalt = no NOT = { has_ethic = ethic_pacifist } is_cool = yes always = no",
        );
        let t = ctx.block(&b).unwrap();
        assert_eq!(t.key, "AND");
        let texts: Vec<_> = t.children.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "Has technology: Red Lasers",
                "Years passed > 20",
                "NOT: Is gestalt",
                "NOT: Has ethic: Pacifist",
                "Is cool",
                "Never"
            ]
        );
        assert_eq!(t.children[4].scripted.as_ref().unwrap()[0].text, "Has ethic: Pacifist");
        assert!(to_lines(&t).starts_with("All of the following\n  Has technology: Red Lasers"));
    }

    #[test]
    fn single_condition_is_not_wrapped() {
        let loc = Localisation::default();
        let scripted = HashMap::new();
        let ctx = TriggerCtx { loc: &loc, scripted: &scripted };
        let (b, _) = parse("has_country_flag = x");
        assert_eq!(ctx.block(&b).unwrap().key, "has_country_flag");
        assert!(ctx.block(&Block::default()).is_none());
    }
}

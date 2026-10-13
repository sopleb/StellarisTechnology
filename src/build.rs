//! The extraction pipeline: game source → [`Output`].

use crate::inline;
use crate::loc::Localisation;
use crate::model::*;
use crate::script::{self, Block, Item, Value};
use crate::source::{GameSource, SourceFile};
use crate::trigger::TriggerCtx;
use crate::vars::Vars;
use std::collections::{BTreeMap, HashMap, HashSet};

/// `common/` folders never scanned for unlocks.
const SKIP_UNLOCK_DIRS: &[&str] = &[
    "common/technology",
    "common/inline_scripts",
    "common/scripted_triggers",
    "common/scripted_effects",
    "common/scripted_variables",
    "common/scripted_loc",
    "common/script_values",
    "common/defines",
    "common/on_actions",
];

/// Localisation key prefixes tried when naming objects from a `common/` folder.
const NAME_PREFIXES: &[(&str, &str)] = &[
    ("starbase_modules", "sm_"),
    ("starbase_buildings", "sm_"),
    ("edicts", "edict_"),
    ("policies", "policy_"),
];

pub struct BuildInput<'a> {
    pub source: &'a GameSource,
    pub lang: &'a str,
    pub game_version: Option<String>,
    pub playset: Option<String>,
    pub warnings: Vec<String>,
}

fn read(f: &SourceFile) -> Block {
    match script::parse_file(&f.path) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("cannot read {}: {e}", f.path.display());
            Block::default()
        }
    }
}

/// Load `*_l_<lang>.yml` from every layer; files under a `replace` folder are applied last.
pub fn load_localisation(src: &GameSource, lang: &str) -> Localisation {
    let suffix = format!("_l_{lang}.yml");
    let files: Vec<SourceFile> = src
        .files_recursive("localisation", "yml")
        .into_iter()
        .chain(src.files_recursive("localisation_synced", "yml"))
        .filter(|f| f.rel.to_lowercase().ends_with(&suffix))
        .collect();
    let is_replace = |f: &SourceFile| f.rel.split('/').any(|c| c == "replace");
    let mut ordered: Vec<&SourceFile> = files.iter().filter(|f| !is_replace(f)).collect();
    ordered.sort_by_key(|f| f.layer);
    ordered.extend(files.iter().filter(|f| is_replace(f)));
    let mut loc = Localisation::default();
    for f in ordered {
        match std::fs::read(&f.path) {
            Ok(b) => loc.load_str(&script::decode(&b)),
            Err(e) => log::warn!("cannot read {}: {e}", f.path.display()),
        }
    }
    loc
}

pub fn build(input: BuildInput) -> Output {
    let src = input.source;
    let mut warnings = input.warnings;
    let loc = load_localisation(src, input.lang);
    log::info!("loaded {} localisation keys", loc.len());

    let mut vars = Vars::default();
    for f in src.files("common/scripted_variables", "txt") {
        vars.add_from(&read(&f));
    }

    let mut scripted = HashMap::new();
    for f in src.files("common/scripted_triggers", "txt") {
        let b = inline::expand(&read(&f), src, &vars);
        for p in b.pairs() {
            if let Some(body) = p.value.as_block() {
                scripted.insert(p.key.clone(), body.clone());
            }
        }
    }
    let ctx = TriggerCtx {
        loc: &loc,
        scripted: &scripted,
    };

    // Techs, in load order; a later definition of the same key replaces the earlier one.
    let mut techs: Vec<Tech> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for f in src.files("common/technology", "txt") {
        let raw = read(&f);
        let fvars = vars.with_file(&raw);
        let block = inline::expand(&raw, src, &fvars);
        for p in block.pairs() {
            let Some(body) = p.value.as_block() else { continue };
            if p.key.starts_with('@') {
                continue;
            }
            let tech = extract_tech(&p.key, body, &fvars, &ctx, src.layer_name(f.layer), &f.rel);
            match index.get(&p.key) {
                Some(&i) => {
                    log::debug!("{} overridden by {} ({})", p.key, tech.file, tech.source);
                    techs[i] = tech;
                }
                None => {
                    index.insert(p.key.clone(), techs.len());
                    techs.push(tech);
                }
            }
        }
    }
    log::info!("extracted {} techs", techs.len());

    scan_unlocks(src, &loc, &mut techs, &index);

    for t in &mut techs {
        dedup(&mut t.unlocks);
    }
    for t in &techs {
        for p in &t.prerequisites {
            if !index.contains_key(p) {
                warnings.push(format!("{}: unknown prerequisite {p}", t.key));
            }
        }
    }

    let categories = extract_categories(src, &loc, &techs);
    let mut tiers: Vec<Tier> = Vec::new();
    for f in src.files("common/technology/tier", "txt") {
        let b = read(&f);
        let fv = vars.with_file(&b);
        for p in b.pairs().filter(|p| !p.key.starts_with('@')) {
            let tier = Tier {
                key: p.key.clone(),
                previously_unlocked: p
                    .value
                    .as_block()
                    .and_then(|b| b.get("previously_unlocked"))
                    .and_then(|v| fv.number_of(v)),
            };
            // A later definition replaces the earlier one in place.
            match tiers.iter_mut().find(|t| t.key == tier.key) {
                Some(t) => *t = tier,
                None => tiers.push(tier),
            }
        }
    }

    let mut areas: Vec<Area> = Vec::new();
    for a in techs.iter().filter_map(|t| t.area.as_ref()) {
        if !areas.iter().any(|x| &x.key == a) {
            areas.push(Area {
                key: a.clone(),
                name: loc.get(a).unwrap_or_else(|| crate::trigger::humanize(a)),
            });
        }
    }

    let mods = src.layers[1..]
        .iter()
        .map(|l| ModInfo {
            name: l.name.clone(),
            path: l.root.display().to_string(),
            remote_id: l.remote_id.clone(),
        })
        .collect();

    Output {
        meta: Meta {
            generator: format!("stellaris-tech {}", env!("CARGO_PKG_VERSION")),
            game_version: input.game_version,
            language: input.lang.to_string(),
            playset: input.playset,
            mods,
            tech_count: techs.len(),
            warnings,
        },
        areas,
        categories,
        tiers,
        techs,
    }
}

fn str_list(b: &Block, key: &str) -> Vec<String> {
    match b.get(key) {
        Some(Value::Block(l)) => l.bare_strings(),
        Some(v) => v.as_str().map(|s| vec![s.to_string()]).unwrap_or_default(),
        None => vec![],
    }
}

pub fn extract_tech(key: &str, b: &Block, vars: &Vars, ctx: &TriggerCtx, source: &str, file: &str) -> Tech {
    let loc = ctx.loc;
    // Scalars, or blocks of the form `{ factor = X modifier = {...} }` (base value = factor/base).
    let num = |k: &str| match b.get(k)? {
        Value::Block(c) => c
            .get("factor")
            .or_else(|| c.get("base"))
            .and_then(|v| vars.number_of(v)),
        v => vars.number_of(v),
    };
    let levels = b.get_str("levels").and_then(|s| s.parse::<i64>().ok());

    let potential = b.get_block("potential");
    let mut requires = BTreeMap::new();
    if let Some(pot) = potential {
        for p in pot.pairs() {
            if let Some(v) = bool_condition(p) {
                requires.insert(p.key.clone(), v);
            } else if p.key == "NOT"
                && let Some(inner) = p.value.as_block()
                && inner.items.len() == 1
                && let Some(ip) = inner.pairs().next()
                && let Some(v) = bool_condition(ip)
            {
                requires.insert(ip.key.clone(), !v);
            }
        }
    }

    let (weight_factor, weight_modifiers) = b
        .get_block("weight_modifier")
        .map(|wm| modifier_list(wm, vars, ctx))
        .unwrap_or_default();
    let cost_modifiers = match b.get("cost") {
        Some(Value::Block(c)) => modifier_list(c, vars, ctx).1,
        _ => vec![],
    };

    let base_weight = num("weight");
    let is_start_tech = b.get_bool("start_tech").unwrap_or(false);
    let is_repeatable = levels.is_some_and(|l| l != 1);
    let unconditional_zero = weight_modifiers.iter().any(|m| {
        m.factor == Some(0.0)
            && m.condition
                .as_ref()
                .is_none_or(|c| c.key == "always" && c.value.as_deref() == Some("yes"))
    });
    // Weight 0 is only "never drawn" when no modifier can add weight back.
    // An `add` we cannot resolve (e.g. `value:...`) is assumed to possibly be positive.
    let can_add = b.get_block("weight_modifier").is_some_and(|wm| {
        wm.get_all("modifier")
            .filter_map(|m| m.value.as_block())
            .filter_map(|m| m.get("add"))
            .any(|a| vars.number_of(a).is_none_or(|n| n > 0.0))
    });
    let is_event = !is_start_tech
        && !is_repeatable
        && ((base_weight == Some(0.0) && !can_add) || weight_factor == Some(0.0) || unconditional_zero);

    let mut unlocks = Vec::new();
    for flag in str_list(b, "feature_flags") {
        let name = loc
            .get(&format!("feature_{flag}"))
            .unwrap_or_else(|| crate::trigger::humanize(&flag));
        unlocks.push(Unlock {
            kind: "feature".into(),
            key: flag,
            name,
        });
    }
    if let Some(m) = b.get_block("modifier") {
        unlocks.extend(modifier_unlocks(m, loc, vars));
    }
    if let Some(pd) = b.get_block("prereqfor_desc") {
        for p in pd.pairs() {
            if let Some(d) = p.value.as_block()
                && let Some(title) = d.get_str("title")
            {
                unlocks.push(Unlock {
                    kind: p.key.clone(),
                    key: title.into(),
                    name: loc.name(title),
                });
            }
        }
    }

    let swaps = b
        .get_all("technology_swap")
        .filter_map(|p| p.value.as_block())
        .filter_map(|s| {
            let k = s.get_str("name")?;
            Some(Swap {
                key: k.into(),
                name: loc.name(k),
                trigger: s.get_block("trigger").and_then(|t| ctx.block(t)),
            })
        })
        .collect();

    Tech {
        key: key.into(),
        name: loc.name(key),
        description: loc.get(&format!("{key}_desc")),
        area: b.get_str("area").map(str::to_string),
        category: str_list(b, "category"),
        tier: b
            .get_str("tier")
            .map(|t| vars.number(t).map_or_else(|| t.to_string(), |n| n.to_string())),
        cost: num("cost"),
        cost_expr: match b.get("cost") {
            Some(Value::Block(c)) => c.get_str("factor").or_else(|| c.get_str("base")).map(str::to_string),
            v => v.and_then(Value::as_str).map(str::to_string),
        },
        cost_modifiers,
        cost_per_level: num("cost_per_level"),
        levels,
        base_weight,
        weight_factor,
        is_start_tech,
        is_rare: b.get_bool("is_rare").unwrap_or(false),
        is_dangerous: b.get_bool("is_dangerous").unwrap_or(false),
        is_repeatable,
        is_insight: b.get_bool("is_insight").unwrap_or(false),
        is_reverse_engineerable: b.get_bool("is_reverse_engineerable"),
        is_event,
        gateway: b.get_str("gateway").map(str::to_string),
        prerequisites: str_list(b, "prerequisites"),
        unlocks,
        requires,
        potential: potential.and_then(|p| ctx.block(p)),
        starting_potential: b.get_block("starting_potential").and_then(|p| ctx.block(p)),
        weight_modifiers,
        swaps,
        source: source.into(),
        file: file.into(),
    }
}

/// Top-level `factor` and the `modifier = { factor/add + conditions }` entries of a block.
fn modifier_list(b: &Block, vars: &Vars, ctx: &TriggerCtx) -> (Option<f64>, Vec<WeightModifier>) {
    let mut top = None;
    let mut out = Vec::new();
    for p in b.pairs() {
        match p.key.as_str() {
            "factor" => top = vars.number_of(&p.value),
            "modifier" => {
                if let Some(m) = p.value.as_block() {
                    let mut cond = Block::default();
                    let (mut factor, mut add) = (None, None);
                    for i in &m.items {
                        match i {
                            Item::Pair(q) if q.key == "factor" => factor = vars.number_of(&q.value),
                            Item::Pair(q) if q.key == "add" => add = vars.number_of(&q.value),
                            _ => cond.items.push(i.clone()),
                        }
                    }
                    out.push(WeightModifier {
                        factor,
                        add,
                        condition: ctx.block(&cond),
                    });
                }
            }
            _ => {}
        }
    }
    (top, out)
}

/// `key = yes|no` (or `!= yes|no`) as the boolean the condition requires.
fn bool_condition(p: &crate::script::Pair) -> Option<bool> {
    let v = p.value.as_bool()?;
    match p.op {
        crate::script::Op::Eq | crate::script::Op::EqEq => Some(v),
        crate::script::Op::Ne => Some(!v),
        _ => None,
    }
}

fn modifier_unlocks(m: &Block, loc: &Localisation, vars: &Vars) -> Vec<Unlock> {
    let mut out = Vec::new();
    for p in m.pairs() {
        match p.key.as_str() {
            "custom_tooltip" | "description" => {
                if let Some(k) = p.value.as_str() {
                    out.push(Unlock {
                        kind: "tooltip".into(),
                        key: k.into(),
                        name: vars.render_refs(&loc.name(k)),
                    });
                }
            }
            k if k.starts_with("show_only_custom_tooltip") || k.starts_with("description") => {}
            k => {
                let label = loc
                    .first(&[format!("mod_{k}"), format!("mod_country_{k}"), k.to_string()])
                    .unwrap_or_else(|| crate::trigger::humanize(k));
                let value = match (vars.number_of(&p.value), p.value.as_str()) {
                    (Some(n), _) => format_modifier_value(k, n),
                    (None, Some(raw)) => raw.to_string(),
                    (None, None) => String::new(),
                };
                out.push(Unlock {
                    kind: "modifier".into(),
                    key: k.into(),
                    name: format!("{label}: {value}"),
                });
            }
        }
    }
    out
}

/// Format a modifier value: `*_mult`/`*_percent` keys are percentages, `*_add` keys are flat,
/// other keys fall back to "fractional means percentage".
pub fn format_modifier_value(key: &str, n: f64) -> String {
    let percent = if key.ends_with("_mult") || key.ends_with("_percent") || key.ends_with("_perc") {
        true
    } else if key.ends_with("_add") {
        false
    } else {
        n.fract() != 0.0
    };
    let sign = if n >= 0.0 { "+" } else { "" };
    if percent {
        format!("{sign}{}%", (n * 10000.0).round() / 100.0)
    } else {
        format!("{sign}{}", (n * 1000.0).round() / 1000.0)
    }
}

fn extract_categories(src: &GameSource, loc: &Localisation, techs: &[Tech]) -> Vec<Category> {
    let mut out: Vec<Category> = Vec::new();
    for f in src.files("common/technology/category", "txt") {
        for p in read(&f).pairs().filter(|p| !p.key.starts_with('@')) {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for t in techs.iter().filter(|t| t.category.contains(&p.key)) {
                if let Some(a) = &t.area {
                    *counts.entry(a).or_default() += 1;
                }
            }
            let area = counts
                .into_iter()
                .max_by_key(|(a, n)| (*n, std::cmp::Reverse(*a)))
                .map(|(a, _)| a.to_string());
            let cat = Category {
                key: p.key.clone(),
                name: loc.name(&p.key),
                area,
            };
            match out.iter_mut().find(|c| c.key == cat.key) {
                Some(c) => *c = cat,
                None => out.push(cat),
            }
        }
    }
    out
}

/// (`common/` folder, object key).
type ObjectId = (String, String);
/// (display name, tech keys it points at).
type ObjectEntry = (String, Vec<String>);

/// Scan every `common/**` object for `prerequisites`/`show_in_tech` naming a tech.
fn scan_unlocks(src: &GameSource, loc: &Localisation, techs: &mut [Tech], index: &HashMap<String, usize>) {
    // (folder, object key) → (display name, tech keys); a later definition replaces an earlier one,
    // mirroring how the game overrides objects redefined in another file.
    let mut objects: Vec<(ObjectId, ObjectEntry)> = Vec::new();
    let mut slot: HashMap<ObjectId, usize> = HashMap::new();
    let mut record = |folder: &str, key: &str, body: &Block| {
        let id = (folder.to_string(), key.to_string());
        let targets = object_targets(body);
        let name = if targets.is_empty() {
            String::new()
        } else {
            object_name(folder, key, loc)
        };
        let entry = (name, targets);
        match slot.get(&id) {
            Some(&i) => objects[i].1 = entry,
            None => {
                slot.insert(id.clone(), objects.len());
                objects.push((id, entry));
            }
        }
    };
    for f in src.files_recursive("common", "txt") {
        if SKIP_UNLOCK_DIRS.iter().any(|d| f.rel.starts_with(&format!("{d}/"))) {
            continue;
        }
        let folder = f.rel.split('/').nth(1).unwrap_or("common").to_string();
        if !f.rel[7..].contains('/') {
            continue; // files directly in common/ are not object folders
        }
        let block = read(&f);
        for p in block.pairs() {
            let Some(body) = p.value.as_block() else { continue };
            let key = body.get_str("key").unwrap_or(&p.key);
            record(&folder, key, body);
            for opt in body.get_all("option").filter_map(|o| o.value.as_block()) {
                if let Some(n) = opt.get_str("name") {
                    record(&folder, n, opt);
                }
            }
        }
    }
    for ((folder, key), (name, targets)) in objects {
        for t in targets {
            if let Some(&i) = index.get(&t) {
                techs[i].unlocks.push(Unlock {
                    kind: folder.clone(),
                    key: key.clone(),
                    name: name.clone(),
                });
            }
        }
    }
}

/// Tech keys an object points at via `prerequisites` / `show_in_tech`.
fn object_targets(body: &Block) -> Vec<String> {
    let mut targets = str_list(body, "prerequisites");
    if let Some(s) = body.get_str("show_in_tech") {
        targets.push(s.to_string());
    }
    let mut seen = HashSet::new();
    targets.retain(|t| seen.insert(t.clone()));
    targets
}

fn object_name(folder: &str, key: &str, loc: &Localisation) -> String {
    let mut candidates = vec![key.to_string()];
    for (dir, prefix) in NAME_PREFIXES {
        if *dir == folder {
            candidates.insert(0, format!("{prefix}{key}"));
        }
    }
    candidates.push(format!("{key}_name"));
    loc.first(&candidates).unwrap_or_else(|| key.to_string())
}

fn dedup(v: &mut Vec<Unlock>) {
    let mut seen = HashSet::new();
    v.retain(|u| seen.insert((u.kind.clone(), u.key.clone())));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_values() {
        assert_eq!(format_modifier_value("x_mult", 0.1), "+10%");
        assert_eq!(format_modifier_value("x", -0.25), "-25%");
        assert_eq!(format_modifier_value("x_add", 3.0), "+3");
        assert_eq!(format_modifier_value("x_add", 0.5), "+0.5");
        assert_eq!(format_modifier_value("x_mult", 1.0), "+100%");
        assert_eq!(format_modifier_value("x", 0.075), "+7.5%");
    }
}

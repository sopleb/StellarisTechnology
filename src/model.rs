//! Output data model, serialized to `techs.json`.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub meta: Meta,
    pub areas: Vec<Area>,
    pub categories: Vec<Category>,
    pub tiers: Vec<Tier>,
    pub techs: Vec<Tech>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub generator: String,
    pub game_version: Option<String>,
    pub language: String,
    pub playset: Option<String>,
    pub mods: Vec<ModInfo>,
    pub tech_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModInfo {
    pub name: String,
    pub path: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Area {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Category {
    pub key: String,
    pub name: String,
    /// Area most techs of this category belong to.
    pub area: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tier {
    pub key: String,
    pub previously_unlocked: Option<f64>,
}

/// A trigger (condition) tree node with a best-effort English rendering.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Trigger {
    pub key: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Trigger>,
    pub text: String,
    /// Body of a scripted trigger, expanded one level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripted: Option<Vec<Trigger>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeightModifier {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<Trigger>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Unlock {
    /// `modifier`, `feature`, `tooltip`, a `prereqfor_desc` kind, or the `common/` folder name.
    pub kind: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Swap {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Trigger>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tech {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub area: Option<String>,
    pub category: Vec<String>,
    pub tier: Option<String>,
    pub cost: Option<f64>,
    pub cost_expr: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cost_modifiers: Vec<WeightModifier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<i64>,
    pub base_weight: Option<f64>,
    pub weight_factor: Option<f64>,
    pub is_start_tech: bool,
    pub is_rare: bool,
    pub is_dangerous: bool,
    pub is_repeatable: bool,
    pub is_insight: bool,
    pub is_reverse_engineerable: Option<bool>,
    pub is_event: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    pub prerequisites: Vec<String>,
    pub unlocks: Vec<Unlock>,
    /// Top-level yes/no conditions of `potential`, e.g. `{"is_gestalt": false}`.
    pub requires: BTreeMap<String, bool>,
    pub potential: Option<Trigger>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_potential: Option<Trigger>,
    pub weight_modifiers: Vec<WeightModifier>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub swaps: Vec<Swap>,
    /// "vanilla" or the defining mod's name.
    pub source: String,
    pub file: String,
}

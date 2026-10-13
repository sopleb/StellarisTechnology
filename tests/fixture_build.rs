use std::path::{Path, PathBuf};
use stellaris_tech::build::{BuildInput, build};
use stellaris_tech::model::{Output, Tech};
use stellaris_tech::mods::layer_from_dir;
use stellaris_tech::source::GameSource;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn run(mods: &[&str], lang: &str) -> Output {
    let mut src = GameSource::new(fixtures().join("game"));
    for m in mods {
        src.push(layer_from_dir(&fixtures().join(m), None).unwrap());
    }
    build(BuildInput {
        source: &src,
        lang,
        game_version: stellaris_tech::discover::game_version(&fixtures().join("game")),
        playset: None,
        warnings: vec![],
    })
}

fn tech<'a>(o: &'a Output, k: &str) -> &'a Tech {
    o.techs
        .iter()
        .find(|t| t.key == k)
        .unwrap_or_else(|| panic!("no tech {k}"))
}

#[test]
fn vanilla_extraction() {
    let o = run(&[], "english");
    assert_eq!(o.meta.game_version.as_deref(), Some("v9.9.9"));
    assert_eq!(
        o.techs.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(),
        ["tech_lasers_1", "tech_lasers_2", "tech_secret", "tech_repeat"]
    );
    assert_eq!(
        o.categories
            .iter()
            .map(|c| (c.key.as_str(), c.name.as_str(), c.area.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("particles", "Particles", Some("physics")),
            ("voidcraft", "voidcraft", Some("society"))
        ]
    );
    assert_eq!(o.tiers.len(), 2);
    assert_eq!(o.tiers[1].previously_unlocked, Some(4.0));
    assert_eq!(o.areas[0].name, "Physics");

    let t = tech(&o, "tech_lasers_2");
    assert_eq!(t.name, "Blue Lasers");
    assert_eq!(t.description.as_deref(), Some("Bluer."));
    assert_eq!((t.cost, t.cost_expr.as_deref()), (Some(1000.0), Some("@tier1cost")));
    assert_eq!(t.base_weight, Some(50.0));
    assert_eq!(t.weight_factor, Some(6.0));
    assert_eq!(t.prerequisites, ["tech_lasers_1"]);
    assert_eq!(t.requires.get("is_gestalt"), Some(&false));
    assert_eq!(t.requires.get("is_machine_empire"), Some(&false));
    assert_eq!(t.weight_modifiers.len(), 2);
    let wm = &t.weight_modifiers[0];
    assert_eq!(wm.factor, Some(0.5));
    let cond = wm.condition.as_ref().unwrap();
    assert_eq!(cond.text, "Is friendly");
    assert_eq!(cond.scripted.as_ref().unwrap()[0].text, "Has ethic: Xenophile");
    assert_eq!(
        t.weight_modifiers[1].condition.as_ref().unwrap().text,
        "Has technology: Red Lasers"
    );
    let unlocks: Vec<(&str, &str)> = t.unlocks.iter().map(|u| (u.kind.as_str(), u.name.as_str())).collect();
    assert_eq!(
        unlocks,
        [
            ("feature", "Better Lasers"),
            ("modifier", "Weapon Damage: +5%"),
            ("tooltip", "Lasers are great"),
            ("component_templates", "Small Blue Laser"),
        ]
    );
    assert_eq!(t.swaps[0].name, "Bio Lasers");
    assert!(!t.is_event);

    let s = tech(&o, "tech_secret");
    assert!(s.is_event);
    assert_eq!(s.cost, Some(500.0));
    assert_eq!(s.cost_modifiers.len(), 1);
    assert_eq!(o.meta.warnings, ["tech_secret: unknown prerequisite tech_missing"]);

    let r = tech(&o, "tech_repeat");
    assert!(r.is_repeatable && !r.is_event);
    assert_eq!((r.cost_per_level, r.levels), (Some(2.0), Some(-1)));

    assert!(
        tech(&o, "tech_lasers_1")
            .unlocks
            .iter()
            .any(|u| u.kind == "buildings" && u.name == "Lab")
    );
    assert_eq!(tech(&o, "tech_lasers_1").source, "vanilla");
}

#[test]
fn mods_override_and_extend() {
    let o = run(&["mod_a", "mod_b"], "english");
    assert_eq!(
        o.meta.mods.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["Mod A", "Mod B"]
    );
    assert_eq!(o.meta.mods[0].remote_id.as_deref(), Some("111"));

    // Same relative file path: mod_a's 00_test_tech.txt replaces vanilla's entirely.
    let keys: Vec<_> = o.techs.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, ["tech_lasers_1", "tech_lasers_2", "tech_mod_a"]);
    let l1 = tech(&o, "tech_lasers_1");
    assert_eq!((l1.cost, l1.source.as_str()), (Some(7.0), "Mod A"));
    assert_eq!(l1.name, "Modded Red Lasers", "replace/ localisation wins");
    assert_eq!(tech(&o, "tech_mod_a").name, "Mod A Tech");
    assert_eq!(tech(&o, "tech_mod_a").file, "common/technology/zz_mod_a.txt");

    // mod_b's replace_path drops vanilla buildings; its own building unlocks tech_mod_a.
    assert!(!l1.unlocks.iter().any(|u| u.kind == "buildings"));
    assert!(tech(&o, "tech_mod_a").unlocks.iter().any(|u| u.key == "building_mod"));
}

#[test]
fn other_language() {
    let o = run(&[], "german");
    assert_eq!(o.meta.language, "german");
    assert_eq!(tech(&o, "tech_lasers_1").name, "Rote Laser");
    assert_eq!(tech(&o, "tech_lasers_2").name, "tech_lasers_2");
}

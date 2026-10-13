//! Smoke tests against a real Stellaris install. Run with `cargo test -- --ignored`.
use std::path::PathBuf;
use stellaris_tech::script;

fn game() -> Option<PathBuf> {
    let p = std::env::var_os("STELLARIS_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs_home().join(".local/share/Steam/steamapps/common/Stellaris"));
    p.join("common").is_dir().then_some(p)
}

fn dirs_home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

#[test]
#[ignore]
fn parses_all_common_files() {
    let Some(g) = game() else { return };
    let mut files = 0;
    let mut bad = Vec::new();
    for e in walkdir::WalkDir::new(g.join("common")).into_iter().flatten() {
        if e.path().extension().is_some_and(|x| x == "txt") {
            files += 1;
            let text = script::decode(&std::fs::read(e.path()).unwrap());
            let (_, errs) = script::parse(&text);
            if !errs.is_empty() {
                bad.push(format!("{} ({}): {:?}", e.path().display(), errs.len(), errs[0]));
            }
        }
    }
    eprintln!("{files} files, {} with errors", bad.len());
    for b in &bad {
        eprintln!("  {b}");
    }
    assert!(files > 1000);
    assert!(
        !bad.iter().any(|b| b.contains("/common/technology/")),
        "technology files must parse cleanly"
    );
}

fn build_real(mods: &[&str]) -> Option<stellaris_tech::model::Output> {
    use stellaris_tech::{build, discover, mods as m, source::GameSource};
    let g = game()?;
    let mut src = GameSource::new(g.clone());
    let ws = discover::workshop_dirs();
    for id in mods {
        src.push(m::layer_from_arg(std::path::Path::new(id), discover::user_dir().as_deref(), &ws).ok()?);
    }
    Some(build::build(build::BuildInput {
        source: &src,
        lang: "english",
        game_version: discover::game_version(&g),
        playset: None,
        warnings: vec![],
    }))
}

/// Independent count: top-level `name = {` lines in technology/*.txt.
fn count_tech_defs(g: &std::path::Path) -> usize {
    let mut n = 0;
    for e in std::fs::read_dir(g.join("common/technology")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "txt") {
            let text = script::decode(&std::fs::read(e.path()).unwrap());
            n += text
                .lines()
                .filter(|l| {
                    let l = l.split('#').next().unwrap_or("").trim_end();
                    l.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && l.contains('=') && l.ends_with('{')
                })
                .count();
        }
    }
    n
}

#[test]
#[ignore]
fn vanilla_acceptance() {
    let Some(o) = build_real(&[]) else { return };
    let g = game().unwrap();
    assert_eq!(o.techs.len(), count_tech_defs(&g));
    assert!(o.meta.warnings.is_empty(), "{:?}", o.meta.warnings);
    let named = o.techs.iter().filter(|t| t.name != t.key).count();
    assert!(named * 100 > o.techs.len() * 95, "only {named} localised");
    assert!(o.techs.iter().all(|t| t.is_repeatable || !t.category.is_empty()));
    assert!(o.techs.iter().all(|t| t.area.is_some()));
    let lasers = o.techs.iter().find(|t| t.key == "tech_lasers_2").unwrap();
    assert_eq!(lasers.prerequisites, ["tech_lasers_1"]);
    assert!(lasers.unlocks.iter().any(|u| u.key == "SMALL_BLUE_LASER"));
}

#[test]
#[ignore]
fn gigastructures_acceptance() {
    let Some(o) = build_real(&["1121692237"]) else { return };
    let giga = o
        .techs
        .iter()
        .filter(|t| t.source.starts_with("Gigastructural"))
        .count();
    assert!(giga > 100, "{giga} giga techs");
    assert_eq!(o.meta.mods.len(), 1);
}

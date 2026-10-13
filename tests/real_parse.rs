//! Smoke tests against a real Stellaris install. Run with `cargo test -- --ignored`.
use std::path::PathBuf;
use stellaris_tech::script;

fn game() -> Option<PathBuf> {
    let p = std::env::var_os("STELLARIS_GAME").map(PathBuf::from).unwrap_or_else(|| {
        dirs_home().join(".local/share/Steam/steamapps/common/Stellaris")
    });
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
    assert!(!bad.iter().any(|b| b.contains("/common/technology/")), "technology files must parse cleanly");
}

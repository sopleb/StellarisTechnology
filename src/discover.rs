//! Locating the Steam install, the Paradox user directory and the game version.

use regex::Regex;
use std::path::{Path, PathBuf};

const GAME_DIR: &str = "steamapps/common/Stellaris";
pub const WORKSHOP_DIR: &str = "steamapps/workshop/content/281990";

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Candidate Steam root directories for the current platform.
pub fn steam_roots() -> Vec<PathBuf> {
    let h = home();
    let mut v = vec![
        h.join(".local/share/Steam"),
        h.join(".steam/steam"),
        h.join(".steam/root"),
        h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        h.join("Library/Application Support/Steam"),
        PathBuf::from("C:/Program Files (x86)/Steam"),
        PathBuf::from("C:/Program Files/Steam"),
    ];
    v.retain(|p| p.is_dir());
    v
}

/// Library paths listed in a `libraryfolders.vdf`.
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
    re.captures_iter(vdf)
        .map(|c| PathBuf::from(c[1].replace("\\\\", "\\")))
        .collect()
}

/// All Steam library folders found from all Steam roots, de-duplicated.
pub fn steam_libraries() -> Vec<PathBuf> {
    let mut libs: Vec<PathBuf> = Vec::new();
    for root in steam_roots() {
        let mut found = vec![root.clone()];
        if let Ok(s) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            found.extend(parse_library_folders(&s));
        }
        for lib in found {
            let canon = lib.canonicalize().unwrap_or(lib);
            if !libs.contains(&canon) {
                libs.push(canon);
            }
        }
    }
    libs
}

pub fn is_game_dir(p: &Path) -> bool {
    p.join("common/technology").is_dir()
}

/// First Steam library containing Stellaris.
pub fn find_game() -> Option<PathBuf> {
    steam_libraries()
        .into_iter()
        .map(|l| l.join(GAME_DIR))
        .find(|p| is_game_dir(p))
}

/// Steam Workshop content directories for Stellaris across all libraries.
pub fn workshop_dirs() -> Vec<PathBuf> {
    steam_libraries()
        .into_iter()
        .map(|l| l.join(WORKSHOP_DIR))
        .filter(|p| p.is_dir())
        .collect()
}

/// The Paradox user directory (`.../Paradox Interactive/Stellaris`).
pub fn user_dir() -> Option<PathBuf> {
    let h = home();
    let mut candidates = Vec::new();
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        candidates.push(PathBuf::from(x).join("Paradox Interactive/Stellaris"));
    }
    candidates.push(h.join(".local/share/Paradox Interactive/Stellaris"));
    candidates.push(h.join("Documents/Paradox Interactive/Stellaris"));
    candidates.into_iter().find(|p| p.is_dir())
}

/// Game version string from `launcher-settings.json`, e.g. "v4.5.2".
pub fn game_version(game: &Path) -> Option<String> {
    let s = std::fs::read_to_string(game.join("launcher-settings.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    v.get("rawVersion")
        .or_else(|| v.get("version"))
        .and_then(|x| x.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vdf() {
        let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
		"apps" { "281990"		"1" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        assert_eq!(
            parse_library_folders(vdf),
            vec![
                PathBuf::from("/home/u/.local/share/Steam"),
                PathBuf::from("D:\\SteamLibrary")
            ]
        );
    }

    #[test]
    fn reads_version() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(
            t.path().join("launcher-settings.json"),
            r#"{"version":"Cygnus v4.5.2","rawVersion":"v4.5.2"}"#,
        )
        .unwrap();
        assert_eq!(game_version(t.path()).as_deref(), Some("v4.5.2"));
    }
}

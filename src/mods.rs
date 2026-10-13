//! Mod descriptors and Paradox launcher playsets.

use crate::script;
use crate::source::Layer;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Descriptor {
    pub name: Option<String>,
    pub path: Option<String>,
    pub remote_file_id: Option<String>,
    pub supported_version: Option<String>,
    pub replace_paths: Vec<String>,
}

pub fn parse_descriptor(text: &str) -> Descriptor {
    let (b, _) = script::parse(text);
    let s = |k: &str| b.get_str(k).map(str::to_string);
    Descriptor {
        name: s("name"),
        path: s("path").or_else(|| s("archive")),
        remote_file_id: s("remote_file_id"),
        supported_version: s("supported_version"),
        replace_paths: b
            .get_all("replace_path")
            .filter_map(|p| p.value.as_str().map(|s| s.replace('\\', "/")))
            .collect(),
    }
}

fn read_descriptor(p: &Path) -> Option<Descriptor> {
    std::fs::read(p).ok().map(|b| parse_descriptor(&script::decode(&b)))
}

/// Build a layer from a mod directory, a `.mod` descriptor file, or a workshop id.
pub fn layer_from_arg(arg: &Path, user_dir: Option<&Path>, workshop: &[PathBuf]) -> Result<Layer> {
    // Bare workshop id.
    if let Some(id) = arg
        .to_str()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
    {
        let dir = workshop.iter().map(|w| w.join(id)).find(|p| p.is_dir());
        return match dir {
            Some(d) => layer_from_dir(&d, None),
            None => bail!("workshop mod {id} not found in any Steam library"),
        };
    }
    if arg.is_dir() {
        return layer_from_dir(arg, None);
    }
    if arg.extension().is_some_and(|e| e == "mod") {
        let d = read_descriptor(arg).with_context(|| format!("reading {}", arg.display()))?;
        let raw = d
            .path
            .clone()
            .with_context(|| format!("{} has no path=", arg.display()))?;
        let mut dir = PathBuf::from(&raw);
        if dir.is_relative() && !looks_absolute(&raw) {
            let base = user_dir
                .map(Path::to_path_buf)
                .or_else(|| arg.parent().and_then(Path::parent).map(Path::to_path_buf));
            dir = base.unwrap_or_default().join(dir);
        }
        return layer_from_dir(&dir, Some(d));
    }
    bail!(
        "mod path {} is neither a directory, a .mod file nor a workshop id",
        arg.display()
    )
}

/// `C:/...` or `C:\...` is absolute even when running on a non-Windows host.
fn looks_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with('/') || (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\'))
}

/// Build a layer from a mod directory, reading its `descriptor.mod` when present.
pub fn layer_from_dir(dir: &Path, outer: Option<Descriptor>) -> Result<Layer> {
    if !dir.is_dir() {
        bail!("mod directory {} does not exist", dir.display());
    }
    let inner = read_descriptor(&dir.join("descriptor.mod")).unwrap_or_default();
    let outer = outer.unwrap_or_default();
    let name = outer.name.or(inner.name).unwrap_or_else(|| {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let mut replace_paths = inner.replace_paths;
    replace_paths.extend(outer.replace_paths);
    Ok(Layer {
        name,
        root: dir.to_path_buf(),
        remote_id: outer.remote_file_id.or(inner.remote_file_id),
        replace_paths,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Playset {
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaysetMod {
    pub name: String,
    pub dir: Option<PathBuf>,
    pub steam_id: Option<String>,
    pub position: i64,
}

fn open_launcher_db(user_dir: &Path) -> Result<rusqlite::Connection> {
    let db = user_dir.join("launcher-v2.sqlite");
    if !db.is_file() {
        bail!("launcher database not found at {}", db.display());
    }
    // Read-only, so a running launcher is never disturbed (WAL contents are still seen).
    let path = db.to_string_lossy().replace('\\', "/");
    let path = path.replace('%', "%25").replace('?', "%3f").replace('#', "%23");
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let uri = format!("file://{path}?mode=ro");
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI;
    rusqlite::Connection::open_with_flags(uri, flags).with_context(|| format!("opening {}", db.display()))
}

/// SQL fragment excluding soft-deleted playsets, when the column exists.
fn not_removed(c: &rusqlite::Connection) -> &'static str {
    let has = c
        .prepare("SELECT 1 FROM pragma_table_info('playsets') WHERE name = 'isRemoved'")
        .and_then(|mut st| st.exists([]))
        .unwrap_or(false);
    if has { " AND COALESCE(isRemoved, 0) = 0" } else { "" }
}

pub fn list_playsets(user_dir: &Path) -> Result<Vec<Playset>> {
    let c = open_launcher_db(user_dir)?;
    let nr = not_removed(&c);
    let mut st = c.prepare(&format!(
        "SELECT name, COALESCE(isActive, 0) FROM playsets WHERE 1 = 1{nr} ORDER BY name"
    ))?;
    let rows = st.query_map([], |r| {
        Ok(Playset {
            name: r.get(0)?,
            active: r.get::<_, i64>(1)? != 0,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Enabled mods of the named playset (or the active one) in load order.
pub fn playset_mods(user_dir: &Path, name: Option<&str>) -> Result<(String, Vec<PlaysetMod>)> {
    let c = open_launcher_db(user_dir)?;
    let nr = not_removed(&c);
    let (id, pname): (String, String) = match name {
        Some(n) => c
            .query_row(
                &format!("SELECT id, name FROM playsets WHERE name = ?1{nr}"),
                [n],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .with_context(|| format!("no playset named {n:?}"))?,
        None => c
            .query_row(
                &format!("SELECT id, name FROM playsets WHERE isActive = 1{nr}"),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .context("no active playset")?,
    };
    let mut st = c.prepare(
        "SELECT COALESCE(m.displayName, m.name, m.id), m.dirPath, m.steamId, COALESCE(pm.position, 0)
         FROM playsets_mods pm JOIN mods m ON m.id = pm.modId
         WHERE pm.playsetId = ?1 AND COALESCE(pm.enabled, 1) = 1
         ORDER BY pm.position",
    )?;
    let rows = st.query_map([&id], |r| {
        Ok(PlaysetMod {
            name: r.get(0)?,
            dir: r.get::<_, Option<String>>(1)?.map(PathBuf::from),
            steam_id: r.get(2)?,
            position: r.get(3)?,
        })
    })?;
    Ok((pname, rows.collect::<rusqlite::Result<_>>()?))
}

/// Layers for a playset; mods whose directory is missing are reported and skipped.
pub fn playset_layers(user_dir: &Path, name: Option<&str>, warnings: &mut Vec<String>) -> Result<(String, Vec<Layer>)> {
    let (pname, mods) = playset_mods(user_dir, name)?;
    let mut layers = Vec::new();
    for m in mods {
        match m.dir.as_deref().map(|d| layer_from_dir(d, None)) {
            Some(Ok(mut l)) => {
                l.name = m.name;
                l.remote_id = l.remote_id.or(m.steam_id);
                layers.push(l);
            }
            Some(Err(e)) => warnings.push(format!("playset mod {:?} skipped: {e}", m.name)),
            None => warnings.push(format!("playset mod {:?} has no directory", m.name)),
        }
    }
    Ok((pname, layers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_descriptor() {
        let d = parse_descriptor(
            "version=\"1\"\ntags={\n\"Gameplay\"\n}\nname=\"Giga (4.5)\"\nsupported_version=\"v4.5.*\"\npath=\"mod/giga\"\nreplace_path=\"common/technology\"\nreplace_path=\"common/buildings\"\nremote_file_id=\"1121692237\"",
        );
        assert_eq!(d.name.as_deref(), Some("Giga (4.5)"));
        assert_eq!(d.path.as_deref(), Some("mod/giga"));
        assert_eq!(d.remote_file_id.as_deref(), Some("1121692237"));
        assert_eq!(d.replace_paths, vec!["common/technology", "common/buildings"]);
    }

    #[test]
    fn layer_from_mod_file_resolves_relative_path() {
        let t = tempfile::tempdir().unwrap();
        let user = t.path();
        std::fs::create_dir_all(user.join("mod/foo")).unwrap();
        std::fs::write(
            user.join("mod/foo/descriptor.mod"),
            "name=\"Inner\"\nreplace_path=\"common/x\"",
        )
        .unwrap();
        std::fs::write(user.join("mod/foo.mod"), "name=\"Outer\"\npath=\"mod/foo\"").unwrap();
        let l = layer_from_arg(&user.join("mod/foo.mod"), Some(user), &[]).unwrap();
        assert_eq!(l.name, "Outer");
        assert_eq!(l.root, user.join("mod/foo"));
        assert_eq!(l.replace_paths, vec!["common/x"]);
    }

    #[test]
    fn windows_paths() {
        assert!(looks_absolute("C:/Users/x/mod"));
        assert!(looks_absolute("Z:\\mods\\x"));
        assert!(!looks_absolute("mod/x"));
        assert_eq!(
            parse_descriptor("replace_path=\"common\\technology\"").replace_paths,
            vec!["common/technology"]
        );
    }

    #[test]
    fn playset_from_sqlite() {
        let t = tempfile::tempdir().unwrap();
        let c = rusqlite::Connection::open(t.path().join("launcher-v2.sqlite")).unwrap();
        c.execute_batch(
            "CREATE TABLE playsets (id TEXT, name TEXT, isActive BOOLEAN, isRemoved BOOLEAN DEFAULT 0);
             CREATE TABLE mods (id TEXT, name TEXT, displayName TEXT, dirPath TEXT, steamId TEXT);
             CREATE TABLE playsets_mods (playsetId TEXT, modId TEXT, enabled BOOLEAN, position INTEGER);
             INSERT INTO playsets VALUES ('p0','A',0,1),('p1','A',0,0),('p2','B',1,0);
             INSERT INTO mods VALUES ('m1','one','One','/x/1','1'),('m2','two','Two','/x/2','2'),('m3','three','Three','/x/3','3');
             INSERT INTO playsets_mods VALUES ('p2','m2',1,1),('p2','m1',1,0),('p2','m3',0,2),('p1','m3',1,0);",
        )
        .unwrap();
        drop(c);
        let ps = list_playsets(t.path()).unwrap();
        assert_eq!(ps.len(), 2, "removed playset is hidden");
        assert_eq!(ps.iter().find(|p| p.active).unwrap().name, "B");
        let (n, mods) = playset_mods(t.path(), None).unwrap();
        assert_eq!(n, "B");
        assert_eq!(
            mods.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            vec!["One", "Two"]
        );
        let (_, a) = playset_mods(t.path(), Some("A")).unwrap();
        assert_eq!(a[0].name, "Three");
    }
}

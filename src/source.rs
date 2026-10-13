//! Layered virtual filesystem: vanilla game files overlaid by mods in load order.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Layer {
    /// Display name: "vanilla" or the mod's name.
    pub name: String,
    pub root: PathBuf,
    pub remote_id: Option<String>,
    /// Directories (relative, `/`-separated) whose earlier-layer contents this layer discards.
    pub replace_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Path relative to the layer root, `/`-separated.
    pub rel: String,
    pub path: PathBuf,
    pub layer: usize,
}

#[derive(Debug, Clone)]
pub struct GameSource {
    pub layers: Vec<Layer>,
}

impl GameSource {
    pub fn new(game: PathBuf) -> Self {
        GameSource {
            layers: vec![Layer {
                name: "vanilla".into(),
                root: game,
                remote_id: None,
                replace_paths: vec![],
            }],
        }
    }

    pub fn game_root(&self) -> &Path {
        &self.layers[0].root
    }

    pub fn push(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub fn layer_name(&self, idx: usize) -> &str {
        &self.layers[idx].name
    }

    /// The winning file for a relative path (`/`-separated), honouring layer order.
    pub fn find(&self, rel: &str) -> Option<SourceFile> {
        let rel = rel.trim_matches('/');
        let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
        (0..self.layers.len()).rev().find_map(|idx| {
            let hidden = self.layers[idx + 1..].iter().any(|l| {
                l.replace_paths
                    .iter()
                    .any(|h| dir == h.trim_matches('/') || dir.starts_with(&format!("{}/", h.trim_matches('/'))))
            });
            let p = self.layers[idx].root.join(rel);
            (!hidden && p.is_file()).then(|| SourceFile {
                rel: rel.to_string(),
                path: p,
                layer: idx,
            })
        })
    }

    /// Files directly inside `dir` with extension `ext`, merged across layers and
    /// sorted by file name (the game's load order within a folder).
    pub fn files(&self, dir: &str, ext: &str) -> Vec<SourceFile> {
        self.collect(dir, ext, false)
    }

    /// Like [`files`](Self::files) but recursive; sorted by relative path.
    pub fn files_recursive(&self, dir: &str, ext: &str) -> Vec<SourceFile> {
        self.collect(dir, ext, true)
    }

    fn collect(&self, dir: &str, ext: &str, recursive: bool) -> Vec<SourceFile> {
        let dir = dir.trim_matches('/');
        let mut merged: BTreeMap<String, SourceFile> = BTreeMap::new();
        for (idx, layer) in self.layers.iter().enumerate() {
            // A later layer's replace_path hides this layer's files beneath it.
            let hidden: Vec<&str> = self.layers[idx + 1..]
                .iter()
                .flat_map(|l| l.replace_paths.iter().map(|s| s.trim_matches('/')))
                .collect();
            let base = layer.root.join(dir);
            if !base.is_dir() {
                continue;
            }
            let walker = walkdir::WalkDir::new(&base)
                .min_depth(1)
                .max_depth(if recursive { usize::MAX } else { 1 });
            for entry in walker.into_iter().flatten() {
                let p = entry.path();
                if !entry.file_type().is_file() || !p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)) {
                    continue;
                }
                let Ok(rel) = p.strip_prefix(&layer.root) else { continue };
                let rel = rel.to_string_lossy().replace('\\', "/");
                let parent = rel.rsplit_once('/').map_or("", |(d, _)| d);
                if hidden
                    .iter()
                    .any(|h| parent == *h || parent.starts_with(&format!("{h}/")))
                {
                    continue;
                }
                merged.insert(
                    rel.clone(),
                    SourceFile {
                        rel,
                        path: p.to_path_buf(),
                        layer: idx,
                    },
                );
            }
        }
        let mut out: Vec<SourceFile> = merged.into_values().collect();
        if !recursive {
            out.sort_by(|a, b| file_name(&a.rel).cmp(file_name(&b.rel)));
        }
        out
    }
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    #[test]
    fn later_layer_overrides_same_path_and_replace_path_hides() {
        let t = tempfile::tempdir().unwrap();
        let (g, m1, m2) = (t.path().join("g"), t.path().join("m1"), t.path().join("m2"));
        write(&g, "common/technology/00_a.txt", "a");
        write(&g, "common/technology/01_b.txt", "b");
        write(&g, "common/buildings/00_x.txt", "x");
        write(&m1, "common/technology/00_a.txt", "a2");
        write(&m1, "common/technology/zz_mod.txt", "z");
        write(&m2, "common/buildings/new.txt", "n");

        let mut src = GameSource::new(g);
        src.push(Layer {
            name: "m1".into(),
            root: m1,
            remote_id: None,
            replace_paths: vec![],
        });
        let tech = src.files("common/technology", "txt");
        assert_eq!(
            tech.iter().map(|f| (f.rel.as_str(), f.layer)).collect::<Vec<_>>(),
            vec![
                ("common/technology/00_a.txt", 1),
                ("common/technology/01_b.txt", 0),
                ("common/technology/zz_mod.txt", 1),
            ]
        );

        src.push(Layer {
            name: "m2".into(),
            root: m2,
            remote_id: None,
            replace_paths: vec!["common/buildings".into()],
        });
        let b = src.files_recursive("common", "txt");
        assert!(b.iter().any(|f| f.rel == "common/buildings/new.txt"));
        assert!(!b.iter().any(|f| f.rel == "common/buildings/00_x.txt"));
        assert_eq!(b.len(), 4);
    }
}

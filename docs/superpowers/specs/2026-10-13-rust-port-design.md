# Plan: port stellaris-technology to Rust, then add mod support

## Context
The Java 8 / Spring / ANTLR tool extracts the Stellaris tech tree into JSON, but it targets the 2.x-era game:
- categories, areas and about 60 trigger names are hard-coded;
- it reads copies of the files from `files/`;
- English only;
- GigaStructures support is hard-wired and never called.

The user wants a Rust port that is **version-agnostic**, followed by **mod support**. They approved overnight autonomous execution once this plan is accepted.

Decisions made with the user:
- **Output:** a single flat `techs.json` with the same information in a new shape. The user updates the viewer.
- **Game files:** auto-detect the Steam install; `--game <path>` overrides it.
- **Triggers:** a structured tree plus a best-effort plain-text rendering.
- **Language:** `--lang`, default `english`.
- **Mods:** from the active Paradox launcher playset (`--playset [name]`) and/or `--mod <path>` flags.
- **Location:** inside this repo, in a standalone repo at `~/localdev/projects/stellaris-technology/stellaris-tech-rs/` (the Java repo is left as a reference). Commits are authored as `sopleb <sopleb@pm.me>`.
- **Test data:** real install, Stellaris 4.5.2 at `~/.local/share/Steam/steamapps/common/Stellaris`.

Verified environment:
- rustc and cargo 1.96 are installed.
- Steam libraries are listed in `~/.local/share/Steam/steamapps/libraryfolders.vdf`.
- Categories and tiers are data files: `common/technology/category/00_category.txt` and `tier/00_tier.txt`.
- Launcher DB: `~/.local/share/Paradox Interactive/Stellaris/launcher-v2.sqlite`.
  - Relevant tables are `playsets(name,isActive)`, `playsets_mods(playsetId,modId,enabled,position)` and `mods(id,name,displayName,dirPath,steamId,...)`.
  - The active playset is "ironman".
- `.mod` descriptors live in `.../Stellaris/mod/*.mod` and use the keys `name`, `path`, `supported_version` and `remote_file_id`.

## Architecture (crate `stellaris-tech`, a binary plus a lib)
```
rust/
  Cargo.toml        deps: clap, serde, serde_json, anyhow, thiserror, rusqlite (bundled), walkdir, log/env_logger; dev: insta or pretty_assertions, tempfile
  src/main.rs       CLI only
  src/lib.rs
  src/script/       Paradox script: lexer.rs, parser.rs, ast.rs (Value: Scalar/Str/Var/Block{entries}, Entry{key,op,value} | bare value)
  src/loc.rs        localisation .yml parser ("key:0 \"text\""), $ref$ resolution, §/£ stripping, replace/ dirs
  src/vars.rs       @variables (scripted_variables + per-file), @[ ] inline math best-effort eval
  src/source.rs     GameSource: layered virtual filesystem (vanilla + ordered mods; same relative path ⇒ later replaces earlier)
  src/discover.rs   Steam library detection (libraryfolders.vdf), Paradox user dir, launcher playset reader (sqlite, read-only), .mod descriptor parser
  src/model.rs      Tech, Trigger, WeightModifier, Unlock, Output structs (serde)
  src/extract/      tech.rs (tech blocks → Tech), unlocks.rs (scan object folders for prerequisites/show_in_tech), categories.rs, triggers.rs (AST → Trigger tree + text)
  src/render.rs     generic trigger→text (localisation-driven, no hard-coded trigger enum; a small optional overrides table)
  src/output.rs     assemble and write techs.json
  tests/            fixtures/ (tiny fake game + fake mods) and integration tests
```

**Parser.** It's a hand-written tokenizer and recursive-descent parser over bytes. It decodes UTF-8 with a Latin-1 fallback and strips a BOM. It handles:
- operators `= == != < > <= >= ?=`;
- `{}` blocks that mix `k=v` entries and bare values;
- `@var` references, `@[ expr ]` inline math, `hsv{}` / `rgb{}`;
- `$PARAM$` and `$PARAM|default$` macros (kept as a literal string);
- `#` comments, quoted strings with escapes, and stray tokens.

It's error-tolerant: when a parse fails it logs the file and line, skips to the next top-level entry and continues. It never panics.

**Version-agnostic rules.**
- Area is a string, read from the tech.
- Categories are read from `common/technology/category` and localised.
- Tiers are read from `tier/`.
- Triggers are kept in generic form:

  ```
  Trigger {key, op, value?, children[], text}
  ```

  `text` is rendered from localisation where a key exists (for example `has_technology` resolves the tech name, `has_ethic` / `has_civic` / `has_authority` and the like localise their value, and scripted triggers are expanded one level from `common/scripted_triggers`). Otherwise it falls back to `key op value`. AND/OR/NOT/NOR/NAND get generic wording.
- Unlock scanning is generic: walk every `common/**` folder, and any top-level object whose block holds `prerequisites = { tech_x }` or `show_in_tech = tech_x` becomes an unlock `{type: <folder name>, key, name}`. This replaces the hard-coded `GameObject` list.
- Modifiers listed in a tech's `modifier = {}` become unlocks of type `modifier`, localised through `mod_<key>`.

**Output (`techs.json`).**
- `meta`: game version from `launcher-settings.json`, language, mods `[{name, path, remote_id}]`, generated_at.
- `areas`: `[{key, name}]`.
- `categories`: `[{key, name, area?}]`.
- `techs`: `[{key, name, description, area, category[], tier, cost, cost_expr, base_weight, weight_factor, is_start_tech, is_rare, is_dangerous, is_repeatable, is_event, is_gestalt?, is_machine?, is_hive?, prerequisites[], unlocks[{type,key,name}], potential: Trigger?, weight_modifiers[{factor?, add?, condition: Trigger?}], source: "vanilla"|<mod name>, file}]`.

The event-tech heuristic is the Java rule, generalised:
- the tech has weight 0 or no prereqs and isn't a start tech;
- or its first weight_modifier is `factor = 0` with the condition `always = no`.

Empire-type flags come from the structured potential trigger (is_gestalt, is_machine_empire, is_hive_empire, plus authority / ethic checks), not from string matching.

**Mods.**
- `GameSource` stacks the layers in this order: vanilla, then the mods in playset `position` order, then any `--mod` flags.
- File override is by relative path.
- For duplicate tech keys the later definition wins, and the tech is tagged with the winning mod.
- Localisation from every layer merges with later-wins precedence, and `localisation/replace/` and `localisation/<lang>/replace/` win over everything.
- Missing prereq keys produce a warning and get recorded in `meta.warnings`. They don't crash the run, which fixes the Java NPE.

**CLI.**
```
stellaris-tech [--game PATH] [--lang english] [--playset [NAME]] [--mod PATH...] [--out techs.json] [-v]
```
`--playset` with no name uses the active playset. `list-playsets` and `list-mods` subcommands help with discovery.

## Execution (TDD, subagent-driven, one commit per task)
0. `git init` at `stellaris-technology/` with a .gitignore covering `target`, `files` and `rust/target`. Write this spec to `docs/superpowers/specs/2026-10-13-rust-port-design.md`. Commit the Java baseline and the spec.
1. Scaffold the crate and its CLI skeleton.
2. Script lexer and parser, with unit tests for every syntax feature listed above. Then a smoke test that parses **every** `.txt` under the real `common/` with zero panics and reports error counts. Target: no errors in `common/technology`.
3. Localisation parser and resolver, with tests. Smoke test: load all English loc.
4. Variables, including `@[ ]` math.
5. Discovery: Steam library detection, `launcher-settings.json` version, Paradox user dir. Tests use fixture vdf files.
6. Tech extraction, categories and tiers. Integration test on a fixture mini-game.
7. Trigger tree and text renderer.
8. Generic unlock scanning.
9. Output assembly and CLI wiring. Then a real run, checked against the acceptance criteria.
   - Sanity check: compare tech names, prereqs and unlocks for about 10 well-known techs (`tech_lasers_2`, `tech_corvettes`, `tech_destroyers`, ...) by hand against the game files, and use the Java `output/*.json` for shape intuition.
10. Mods: `.mod` descriptor parser, launcher sqlite playset reader (opened read-only and immutable, so a running launcher isn't disturbed), layered `GameSource`, merge rules. Fixture tests for override precedence. Then a real run with `--playset`, which uses the active "ironman" playset and includes GigaStructures 1121692237 if it's in that playset (otherwise `--mod` with its workshop path).
11. `rust/README.md` with usage and a description of the output schema. A final `cargo clippy -- -D warnings` and `cargo fmt`, then commit.

The code-review skill runs after each major phase (after 4, 9 and 10), and confirmed issues get fixed.

## Verification / acceptance
- `cargo test` and `cargo clippy -- -D warnings` both pass.
- `cargo run --release -- --out out/techs.json` (game path auto-detected):
  - exits 0;
  - the tech count equals the number of top-level `tech_*` keys counted independently by grepping `common/technology/*.txt`, excluding `@var` lines;
  - every prerequisite resolves, or appears in `meta.warnings`;
  - more than 95% of techs have a localised name, meaning name ≠ key;
  - categories are non-empty for every non-repeatable tech.
- A `--playset` run exits 0 and adds techs whose `source` is the mod name. `meta.mods` lists the playset in load order.
- A `--lang german` run produces German names.
- A morning summary (written to `rust/NIGHTLY_REPORT.md` and given in chat) covers counts, warnings, known gaps and the commit list.

## Out of scope
- Viewer and frontend changes.
- Exact JSON compatibility with the Java output.
- Deleting the Java code.
- Pushing to any remote.

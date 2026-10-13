# Design: port stellaris-technology to Rust, then add mod support

This is the plan we agreed on 2026-10-13, before any code existed. The README describes the tool as built. The build differs from this plan in a few places:
- The code sits in flat modules (`build.rs`, `trigger.rs`, `inline.rs`) instead of `extract/`, `render.rs` and `output.rs`.
- The launcher database opens read-only without `immutable=1`, so the tool sees changes the launcher hasn't checkpointed yet.
- `is_event` uses a broader rule (see NIGHTLY_REPORT.md), and `requires` replaces the separate `is_gestalt` / `is_machine` / `is_hive` flags.
- `prerequisite_groups` holds `OR = {}` prerequisites. The plan didn't know about them.
- The viewer moved into scope later and lives in `site/`.

## Context
The Java 8 / Spring / ANTLR tool extracts the Stellaris tech tree into JSON, but it targets the 2.x game:
- It hard-codes categories, areas and about 60 trigger names.
- It reads copies of the game files from `files/`.
- It only handles English.
- Its GigaStructures support exists but nothing calls it.

We want a Rust port that reads all of this from the game files, so it works on new game versions, and then mod support. You approved overnight autonomous work once this plan was accepted.

Decisions:
- **Output:** one flat `techs.json` with the same information in a new shape. The viewer gets updated to match.
- **Game files:** find the Steam install automatically, and let `--game <path>` override it.
- **Triggers:** a structured tree plus a plain-text rendering.
- **Language:** `--lang`, default `english`.
- **Mods:** from the active Paradox launcher playset (`--playset [name]`) and from `--mod <path>` flags.
- **Location:** a standalone repo at `~/localdev/projects/stellaris-technology/stellaris-tech-rs/`. The Java repo stays as a reference. Commits use `sopleb <sopleb@pm.me>`.
- **Test data:** the real install, Stellaris 4.5.2 at `~/.local/share/Steam/steamapps/common/Stellaris`.

What I checked on this machine:
- rustc and cargo 1.96 are installed.
- `~/.local/share/Steam/steamapps/libraryfolders.vdf` lists the Steam libraries.
- Categories and tiers live in data files: `common/technology/category/00_category.txt` and `tier/00_tier.txt`.
- The launcher database is `~/.local/share/Paradox Interactive/Stellaris/launcher-v2.sqlite`.
  - The tables we need are `playsets(name,isActive)`, `playsets_mods(playsetId,modId,enabled,position)` and `mods(id,name,displayName,dirPath,steamId,...)`.
  - The active playset is "ironman".
- `.mod` descriptors live in `.../Stellaris/mod/*.mod` and use the keys `name`, `path`, `supported_version` and `remote_file_id`.

## Architecture (crate `stellaris-tech`, a binary plus a lib)
```
stellaris-tech-rs/  (first planned as rust/ inside the Java repo; moved to its own repo at your request)
  Cargo.toml        deps: clap, serde, serde_json, anyhow, thiserror, rusqlite (bundled), walkdir, log/env_logger; dev: insta or pretty_assertions, tempfile
  src/main.rs       CLI only
  src/lib.rs
  src/script/       Paradox script: lexer.rs, parser.rs, ast.rs (Value: Scalar/Str/Var/Block{entries}, Entry{key,op,value} | bare value)
  src/loc.rs        localisation .yml parser ("key:0 \"text\""), $ref$ resolution, §/£ stripping, replace/ dirs
  src/vars.rs       @variables (scripted_variables + per-file), @[ ] inline math
  src/source.rs     GameSource: layered virtual filesystem (vanilla + ordered mods; a later layer replaces the same relative path)
  src/discover.rs   Steam library detection (libraryfolders.vdf), Paradox user dir, launcher playset reader (sqlite, read-only), .mod descriptor parser
  src/model.rs      Tech, Trigger, WeightModifier, Unlock, Output structs (serde)
  src/extract/      tech.rs (tech blocks → Tech), unlocks.rs (scan object folders for prerequisites/show_in_tech), categories.rs, triggers.rs (AST → Trigger tree + text)
  src/render.rs     trigger → text from localisation, with no hard-coded trigger enum and a small optional overrides table
  src/output.rs     assemble and write techs.json
  tests/            fixtures/ (tiny fake game + fake mods) and integration tests
```

**Parser.** A hand-written tokenizer and recursive-descent parser. It decodes UTF-8, falls back to Latin-1, and strips a BOM. It handles:
- the operators `= == != < > <= >= ?=`;
- `{}` blocks that mix `k=v` entries and bare values;
- `@var` references, `@[ expr ]` inline math, `hsv{}` and `rgb{}`;
- `$PARAM$` and `$PARAM|default$` macros, kept as literal strings;
- `#` comments, quoted strings with escapes, and stray tokens.

When a parse fails, the parser logs the file and line, skips to the next top-level entry and continues. It must not panic.

**Reading game data instead of hard-coding it.**
- The tool reads each tech's area as a plain string.
- It reads categories from `common/technology/category` and localises them.
- It reads tiers from `tier/`.
- It keeps triggers as trees:

  ```
  Trigger {key, op, value?, children[], text}
  ```

  The tool builds `text` from localisation when the game has a key for it. `has_technology` shows the tech's name, `has_ethic`, `has_civic` and `has_authority` localise their values, and scripted triggers expand one level from `common/scripted_triggers`. Without a key, `text` falls back to `key op value`. AND, OR, NOT, NOR and NAND get fixed wording.
- The tool walks every `common/**` folder. A top-level object whose block holds `prerequisites = { tech_x }` or `show_in_tech = tech_x` becomes an unlock `{type: <folder name>, key, name}`. This replaces the Java tool's hard-coded `GameObject` list.
- Modifiers in a tech's `modifier = {}` become unlocks of type `modifier`, named through `mod_<key>`.

**Output (`techs.json`).**
- `meta`: the game version from `launcher-settings.json`, the language, the mods `[{name, path, remote_id}]` and generated_at.
- `areas`: `[{key, name}]`.
- `categories`: `[{key, name, area?}]`.
- `techs`: `[{key, name, description, area, category[], tier, cost, cost_expr, base_weight, weight_factor, is_start_tech, is_rare, is_dangerous, is_repeatable, is_event, is_gestalt?, is_machine?, is_hive?, prerequisites[], unlocks[{type,key,name}], potential: Trigger?, weight_modifiers[{factor?, add?, condition: Trigger?}], source: "vanilla"|<mod name>, file}]`.

The event-tech rule generalises the Java one. A tech is an event tech when either:
- it isn't a start tech and has weight 0 or no prerequisites;
- its first weight_modifier is `factor = 0` with the condition `always = no`.

The empire-type flags come from the potential trigger tree (is_gestalt, is_machine_empire, is_hive_empire, plus authority and ethic checks). String matching would break on reworded text.

**Mods.**
- `GameSource` stacks the layers: vanilla, then the mods in playset `position` order, then any `--mod` flags.
- A file in a later layer replaces the earlier file at the same relative path.
- When two layers define the same tech key, the later definition wins, and the tech records the winning mod.
- The tool merges localisation from every layer, with later layers winning. `localisation/replace/` and `localisation/<lang>/replace/` win over everything.
- A prerequisite that names a missing tech produces a warning in `meta.warnings` and doesn't stop the run. The Java tool crashed with a null pointer exception here.

**CLI.**
```
stellaris-tech [--game PATH] [--lang english] [--playset [NAME]] [--mod PATH...] [--out techs.json] [-v]
```
`--playset` with no name uses the active playset. The `list-playsets` and `list-mods` subcommands show what's available.

## Execution (TDD, subagent-driven, one commit per task)
0. `git init` at `stellaris-technology/` with a .gitignore covering `target`, `files` and `rust/target`. Write this spec to `docs/superpowers/specs/2026-10-13-rust-port-design.md`. Commit the Java baseline and the spec.
1. Scaffold the crate and its CLI skeleton.
2. Script lexer and parser, with unit tests for each syntax feature above. Then a smoke test that parses each `.txt` under the real `common/` without panicking and reports error counts. Target: no errors in `common/technology`.
3. Localisation parser and resolver, with tests. Smoke test: load all English localisation.
4. Variables, including `@[ ]` math.
5. Discovery: Steam library detection, the `launcher-settings.json` version, the Paradox user dir. Tests use fixture vdf files.
6. Tech extraction, categories and tiers. Integration test on a fixture mini-game.
7. Trigger tree and text renderer.
8. Unlock scanning across all `common/` folders.
9. Output assembly and CLI wiring. Then a real run, checked against the acceptance criteria below.
   - Sanity check: compare names, prerequisites and unlocks for about 10 well-known techs (`tech_lasers_2`, `tech_corvettes`, `tech_destroyers`, ...) against the game files by hand. Use the Java `output/*.json` as a guide to the shape.
10. Mods: the `.mod` descriptor parser, the launcher sqlite playset reader (read-only and immutable, so a running launcher keeps working), the layered `GameSource` and the merge rules. Fixture tests for override precedence. Then a real `--playset` run on the active "ironman" playset. If GigaStructures (1121692237) isn't in that playset, load it with `--mod` from its workshop path.
11. `README.md` with usage and the output schema. A final `cargo clippy -- -D warnings` and `cargo fmt`, then commit.

Run the code-review skill after phases 4, 9 and 10, and fix the issues it confirms.

## Verification and acceptance
- `cargo test` and `cargo clippy -- -D warnings` pass.
- `cargo run --release -- --out out/techs.json`, with the game path found automatically:
  - exits 0;
  - produces as many techs as a grep counts top-level `tech_*` keys in `common/technology/*.txt`, excluding `@var` lines;
  - resolves each prerequisite or lists it in `meta.warnings`;
  - gives more than 95% of techs a localised name (name ≠ key);
  - gives every non-repeatable tech at least one category.
- A `--playset` run exits 0 and adds techs whose `source` is the mod name. `meta.mods` lists the playset in load order.
- A `--lang german` run produces German names.
- A morning summary in `NIGHTLY_REPORT.md`, repeated in chat, covers counts, warnings, known gaps and the commit list.

## Out of scope
- Viewer and frontend changes.
- Exact JSON compatibility with the Java output.
- Deleting the Java code.
- Pushing to any remote.

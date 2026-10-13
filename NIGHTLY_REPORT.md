# Nightly report (2026-10-13)

## What I built
`stellaris-tech` ports the Java stellaris-technology extractor to Rust and adds mod support. It lives in its own repo, `stellaris-tech-rs`, on branch `main`.

- **Parser:** a hand-written Paradox script parser that recovers from errors. It parses 2062 of the 2063 `common/**.txt` files in 4.5.2. The one it rejects is `HOW_TO_MAKE_NEW_SHIPS.txt`, a documentation file.
- **Extraction without hard-coded game data:**
  - Categories and tiers come from the data files.
  - Trigger conditions become trees, labelled from localisation.
  - Unlocks come from a scan of every `common/` object for `prerequisites` and `show_in_tech`.
- **Script features:** `inline_script` expansion (parameters, defaults, computed script names) and `@var` / `@[ math ]` resolution.
- **Mods:** the tool reads the launcher playset from its sqlite database (read-only) and takes `--mod` folders, `.mod` files or workshop ids. It honours `replace_path` and `localisation/replace/`. It finds Steam on its own, and `--lang` picks the language.
- **Viewer:** `site/` shows the output as cards or as a prerequisite graph, ready for GitHub Pages (see the README).

## Results on this machine (Stellaris v4.5.2)
| Run | Techs | Warnings | Time |
|---|---|---|---|
| vanilla | 679 (matches an independent count of definitions) | 0 | ~0.25s |
| `--mod 1121692237` (GigaStructures) | 983 (306 from Giga) | 4 | ~0.3s |
| `--playset "Ep3o's Essential Mods -4.3 fix"` (36 mods) | 1286 | 4 | ~0.4s |
| `--playset` (active "ironman", UI-only mods) | 679 | 0 | |
| `--lang german` | 679, German names | 0 | |

The 4 warnings come from GigaStructures prerequisites that name ACOT techs. ACOT is a separate mod that I didn't load.

## Tests
- `cargo test` runs 32 unit tests and 4 fixture integration tests. The fixtures hold a mini game and 2 mods, covering overrides, `replace_path`, `replace/` localisation, German, events, cost blocks, swaps and inline scripts.
- `cargo test -- --ignored` runs 3 acceptance tests against the real install. They check that:
  - the tech count equals the definition count;
  - vanilla produces no warnings;
  - more than 95% of names are localised;
  - every non-repeatable tech has a category;
  - GigaStructures loads.
- `cargo clippy --all-targets -- -D warnings` passes.
- A code review found no panics, 7 real bugs and a few minor ones. I fixed all of them and added regression tests.

## Fixed after the first report
- **Localisation keys are case-sensitive.** `astral_threads` and `ASTRAL_THREADS` hold different text. The tool now tries the exact key first, which removed names like `$NEUROCHIPS$ (Siege)`.
- **Runtime placeholders:** the tool drops placeholders such as `$CIVIC|Y$` from condition text and fills `$@var|0%$` in effect text from script variables.
- **"One of" prerequisites:** `prerequisites` can hold `OR = { … }` groups. The tool used to drop them, which left Titans and Juggernaut with no prerequisites. 19 vanilla techs use these groups. The output now lists them in `prerequisite_groups`.

## Known gaps and judgement calls
- **Weaver and biogenesis techs:** about 40 of them list no unlocks. The game grants them through scripted effects (`give_technology`, `add_research_option`), and the scan only follows `prerequisites`.
- **Duplicate tech or object keys across files:** the definition loaded last (by file name) wins. The Java tool does the same, and it matches the usual "last in, only served" behaviour. I haven't checked each object type against the game.
- **Localisation outside `replace/`:** a mod's localisation also overrides vanilla keys, because the later layer wins.
- **Trigger text:** the wording stays generic ("Has ethic: Militarist", "NOT: Is gestalt"). The Java tool used hand-written English. The JSON keeps the full tree, so a viewer can word it differently.
- **`is_event`:** a heuristic. A tech counts as an event tech when it is neither a start tech nor repeatable, and one of these holds:
  - its weight is 0 and no modifier can add to it;
  - its weight factor is 0;
  - an unconditional `factor = 0` modifier applies.

  Vanilla 4.5.2 has 106 such techs.
- **Modifier values:** keys ending in `_mult`, `_percent` or `_perc` show as percentages, and `_add` keys show as flat numbers. Any other key shows as a percentage when its value is fractional.
- **Leftover placeholder:** "Species Modification Points: $POINTS|0=+$" keeps its placeholder, because the game fills in that number during play.

## Housekeeping
- **Java repo:** before the move, I made one commit (`0bd693f`) on `master` in the Java repo at `../stellaris-technology`. It adds the design spec and 3 `.gitignore` lines. Run `git reset --hard HEAD~1` there to drop it.
- **Nothing pushed:** this repo has no remote yet.

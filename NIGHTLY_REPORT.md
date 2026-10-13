# Nightly report (2026-10-13)

## What was built
`stellaris-tech` is a Rust port of the Java stellaris-technology extractor, with mod support added. It has its own repo, `stellaris-tech-rs` (branch `main`).

- **Parser:** a hand-written, error-tolerant Paradox script parser. It parses 2062 of the 2063 `common/**.txt` files in 4.5.2 cleanly; the one failure is the `HOW_TO_MAKE_NEW_SHIPS.txt` doc file.
- **Version-agnostic extraction:**
  - categories and tiers are read from data files;
  - trigger conditions become generic trees with text rendered from localisation;
  - unlocks are found by scanning every `common/` object for `prerequisites` / `show_in_tech`.
- **Script features:** `inline_script` expansion (parameters, defaults, computed script names) and `@var` / `@[ math ]` resolution.
- **Mods:** loaded from the launcher playset (read from sqlite, read-only), from `--mod` dirs, `.mod` files or workshop ids, with `replace_path` and `localisation/replace/` support. Steam is auto-detected. `--lang` selects the language.

## Results on this machine (Stellaris v4.5.2)
| Run | Techs | Warnings | Time |
|---|---|---|---|
| vanilla | 679 (matches an independent count of definitions) | 0 | ~0.25s |
| `--mod 1121692237` (GigaStructures) | 983 (306 from Giga) | 4 | ~0.3s |
| `--playset "Ep3o's Essential Mods -4.3 fix"` (36 mods) | 1286 | 4 | ~0.4s |
| `--playset` (active "ironman", UI-only mods) | 679 | 0 | |
| `--lang german` | 679, German names | 0 | |

All 4 warnings are GigaStructures prerequisites that point at ACOT techs (another mod, not loaded). They're expected.

## Tests
- `cargo test`: 28 unit tests and 4 fixture integration tests (a mini game plus 2 mods covering overrides, `replace_path`, `replace/` localisation, German, events, cost blocks, swaps and inline scripts).
- `cargo test -- --ignored`: 3 acceptance tests against the real install:
  - the tech count equals the definition count;
  - there are no warnings;
  - more than 95% of names are localised;
  - every non-repeatable tech has a category;
  - GigaStructures loads.
- `cargo clippy --all-targets -- -D warnings` is clean.
- An independent code review found no panics and 7 real issues plus minor ones. All are fixed, with regression tests.

## Known gaps / judgement calls
- **Weaver and biogenesis techs:** about 40 of them list no unlocks, because the game links them through scripted effects (`give_technology`, `add_research_option`) rather than `prerequisites`.
- **Duplicate tech or object keys across files:** the definition loaded last (by file name) wins. This matches the Java tool and the usual "LIOS" behaviour, but I haven't verified it per object type against the game.
- **Localisation without `replace/`:** a mod's localisation outside `replace/` also overrides vanilla keys (later layer wins).
- **Trigger text:** generic ("Has ethic: Militarist", "NOT: Is gestalt"). It isn't the Java tool's hand-written English. The structured tree is in the JSON for the viewer to render however it likes.
- **`is_event`:** a heuristic. True when the tech isn't a start tech and isn't repeatable, and weight is 0 with no possible `add`, or the weight factor is 0, or an unconditional `factor = 0` modifier applies. Vanilla 4.5.2 gives 106 such techs.
- **Modifier values:** keys ending in `_mult`, `_percent` or `_perc` are shown as percentages, `_add` keys as flat numbers, and anything else is a percentage only when the value is fractional.
- **Viewer:** not touched. The output schema is new (see README).

## Housekeeping
- **Java repo:** before the move, I made one commit (`0bd693f`) on `master` in the Java repo at `../stellaris-technology` (the design spec and 3 extra `.gitignore` lines). It's still there. Drop it with `git reset --hard HEAD~1` if you don't want it.
- **Nothing pushed:** this repo has no remote.

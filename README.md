# stellaris-tech

`stellaris-tech` reads the Stellaris technology tree from your game install, plus any mods you load, and writes it to one flat `techs.json`. `site/` holds a viewer for that file.

It's a Rust port of the Java [stellaris-technology](https://github.com/BloodStainedCrow/stellaris-technology) extractor. The Java tool hard-coded categories and trigger names for the 2.x game. This port reads them from the game files:
- It takes areas, categories and tiers from `common/technology`.
- It turns trigger conditions into trees and labels them with the game's own localisation.
- It finds unlocks by scanning every `common/` folder for objects that name a tech in `prerequisites` or `show_in_tech`.

On Stellaris 4.5.2 the vanilla run extracts 679 techs in about 0.25s. GigaStructures adds 306 techs, and a 36-mod playset gives 1286.

## Usage

```sh
cargo build --release
B=target/release/stellaris-tech

$B                                   # vanilla, english → techs.json (finds the game through Steam)
$B --pretty -o out/techs.json        # pretty-printed, custom path
$B --lang german                     # any localisation language folder
$B --playset                         # mods of the launcher's active playset
$B --playset "My Playset"            # a named playset
$B --mod 1121692237                  # a workshop id
$B --mod ~/mods/my_mod --mod x.mod   # mod folders or .mod descriptors, loaded after the playset in order
$B list-playsets                     # * marks the active one
$B list-mods ["Playset name"]
$B -v / -vv                          # info / debug logging (RUST_LOG also works)
```

Use `--game` to point at a game folder yourself. `--user-dir` sets the Paradox user folder, the one that holds `launcher-v2.sqlite` and `mod/`. The tool opens the launcher database read-only, so a running launcher keeps working.

## Mod loading rules

- **Layer order:** vanilla first, then the playset mods in launcher order, then each `--mod` in the order you gave them.
- **Files:** a file in a later layer replaces the earlier file at the same relative path. A descriptor's `replace_path="common/x"` hides every earlier layer's files under `common/x`.
- **Load order inside a folder:** the tool merges files from all layers and loads them in ASCII order of file name, as the game does.
- **Duplicate keys:** when two files define the same key, the folder decides which one counts. The rules come from the "Overwriting specific elements" table on the [Stellaris wiki's Modding page](https://stellaris.paradoxwikis.com/Modding). The game doesn't document them, and the wiki last checked them on 2.5.
  - **Last definition wins (LIOS):** `technology`, `buildings`, `scripted_triggers` and most other folders. A tech's `source` names the layer whose definition won. If the winning tech has no `potential` block, it keeps the one from the tech it replaced, as the game does.
  - **First definition wins (FIOS):** `component_templates`, `section_templates`, `component_sets`, `global_ship_designs`, `special_projects`, `scripted_variables` and a few others. A mod that redefines `SENSOR_1` in `PrimPlayers_00_utilities_sensors.txt` loses to vanilla's `00_utilities_sensors.txt`, because `0` sorts before `P`.
  - **Both stay (DUPL):** `traits`, `strategic_resources` and `name_lists`. The tool merges both definitions' unlocks.
- **Localisation:** the tool loads each layer's `*_l_<lang>.yml` files in layer order, then applies files under any `replace/` folder last.
- **`inline_script`:** the tool expands inline scripts, including parameters, `$P|default$` defaults and computed script names such as `case_@[ x ]`.
- **Unknown prerequisites:** a prerequisite that names a missing tech, such as one from a mod you didn't load, goes into `meta.warnings`. The run carries on.

## Output schema

```jsonc
{
  "meta": {
    "generator": "stellaris-tech 0.1.0", "game_version": "v4.5.2", "language": "english",
    "playset": null, "mods": [{ "name", "path", "remote_id" }], "tech_count": 679,
    "warnings": ["tech_x: unknown prerequisite tech_y"]
  },
  "areas":      [{ "key": "physics", "name": "Physics" }],
  "categories": [{ "key": "particles", "name": "Particles", "area": "physics" }],
  "tiers":      [{ "key": "1", "previously_unlocked": 0 }],
  "techs": [{
    "key": "tech_lasers_2", "name": "Blue Lasers", "description": "...",
    "area": "physics", "category": ["particles"], "tier": "1",
    "cost": 1250, "cost_expr": "@tier1cost2",
    "cost_modifiers": [],                   // omitted when empty; same shape as weight_modifiers
    "cost_per_level": null, "levels": null, // repeatables only
    "base_weight": 95, "weight_factor": null,
    "is_start_tech": false, "is_rare": false, "is_dangerous": false, "is_repeatable": false,
    "is_insight": false, "is_reverse_engineerable": null,
    "is_event": false,                      // the game never offers it as a random research option
    "gateway": null,
    "prerequisites": ["tech_lasers_1"],     // you need all of these
    "prerequisite_groups": [["tech_battleships", "tech_stinger_growth_2"]], // OR = {} groups: you need one tech from each; omitted when empty
    "unlocks": [{ "kind": "component_templates", "key": "SMALL_BLUE_LASER", "name": "Small Blue Laser" }],
    "requires": { "is_gestalt": false },    // top-level yes/no conditions of `potential`
    "potential": Trigger | null,
    "starting_potential": Trigger,          // omitted when absent
    "weight_modifiers": [{ "factor": 1.25, "add": null, "condition": Trigger }],
    "swaps": [{ "key", "name", "trigger": Trigger }],  // technology_swap variants, omitted when empty
    "source": "vanilla",                    // or the mod name
    "file": "common/technology/00_phys_weapon_tech.txt"
  }]
}
```

`unlocks[].kind` tells you where an unlock came from:

| Kind | Source |
|---|---|
| `modifier` | the tech's `modifier = {}` |
| `tooltip` | `custom_tooltip` |
| `feature` | `feature_flags` |
| `prereqfor_desc` kind (`ship`, `custom`, ...) | `prereqfor_desc` |
| `common/` folder name (`component_templates`, `buildings`, ...) | an object whose `prerequisites` or `show_in_tech` names the tech |

A `Trigger` node:

```jsonc
{ "key": "has_ethic", "op": "=", "value": "ethic_militarist", "text": "Has ethic: Militarist",
  "children": [ ... ],      // for blocks (AND/OR/NOT/scopes); omitted when empty
  "scripted": [ ... ] }     // scripted trigger body, expanded one level; omitted otherwise
```

`AND`, `OR`, `NOT`, `NOR` and `NAND` get fixed wording. Other keys take their label from the localisation key `trigger_<key>`. If the game has no such key, the tool turns the key into words (`has_ethic` becomes "Has ethic"). Values get localised names when the game has one, so `has_technology = tech_x` shows the tech's name.

## Viewer

`site/` is a static viewer for `techs.json`. It has no build step and no dependencies apart from Google Fonts (Lilex), and the page falls back to system fonts. You can host it on GitHub Pages and open it in the Steam overlay browser.

- **Cards:** techs grouped by area and tier.
- **Graph:** prerequisite chains drawn left to right, with pan and zoom (drag, wheel, pinch, arrow keys and `+`/`-`/`0`). Pick a tech and the graph lights up what it needs in teal and what it leads to in amber. Dashed lines mark "one of" alternatives.
- **Detail panel:**
  - cost, weight, prerequisites, and the full chain with the research it costs;
  - unlocks and empire requirements;
  - condition trees, weight and cost modifiers, and variants.
- **Search and filters:**
  - Search covers names, keys and unlock names. `/` jumps to the search box, and Enter selects the first match.
  - Filters cover tier, category, rare, dangerous, starting, insight, event techs, repeatables and source mod.
- **Shareable links:** the URL keeps the dataset, view, area, selected tech and search, for example `#ds=vanilla&view=graph&tech=tech_destroyers`.
- **Your own data:** "Open file…" loads a `techs.json` you generated, such as your own playset. The file stays in your browser.

```sh
scripts/build-site-data.sh                 # regenerate site/data/*.json + datasets.json from the local install
python3 -m http.server -d site 8000        # preview at http://localhost:8000
```

Opening `index.html` straight from disk won't load the data, because browsers block `fetch` on `file://` pages. Serve the folder over HTTP as shown above.

To add a dataset, append a `gen <id> <label> <args…>` line to `scripts/build-site-data.sh`. `.github/workflows/pages.yml` publishes `site/` on each push to `main` that changes it. Set Pages → Source to "GitHub Actions" in the repository settings. The workflow can't run the extractor, since GitHub has no game install. Regenerate `site/data/` on your machine and commit it.

## Development

```sh
cargo test                    # unit + fixture integration tests (tests/fixtures: mini game + two mods)
cargo test -- --ignored       # acceptance tests against your install (set STELLARIS_GAME to use another path)
cargo clippy --all-targets -- -D warnings
```

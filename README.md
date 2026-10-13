# stellaris-tech

Extracts the Stellaris technology tree from vanilla and modded game files into a single flat `techs.json`.

It's a Rust port of the Java [stellaris-technology](https://github.com/BloodStainedCrow/stellaris-technology) extractor and is version-agnostic:
- Areas, categories and tiers are read from the game files.
- Trigger conditions become generic trees, and their text is rendered from localisation.
- Unlocks are found by scanning every `common/` folder for objects that reference a tech.

Tested against Stellaris 4.5.2: 679 vanilla techs extract in about 0.25s. It also handles GigaStructures (306 techs) and a 36-mod playset with 1286 techs.

## Usage

```sh
cargo build --release
B=target/release/stellaris-tech

$B                                   # vanilla, english → techs.json (game auto-detected from Steam)
$B --pretty -o out/techs.json        # pretty-printed, custom path
$B --lang german                     # any localisation language folder
$B --playset                         # mods of the launcher's active playset
$B --playset "My Playset"            # a named playset
$B --mod 1121692237                  # a workshop id
$B --mod ~/mods/my_mod --mod x.mod   # mod dirs / .mod descriptors, loaded after the playset in order
$B list-playsets                     # * marks the active one
$B list-mods ["Playset name"]
$B -v / -vv                          # info / debug logging (RUST_LOG also works)
```

`--game` overrides the Steam auto-detection, and `--user-dir` overrides the Paradox user directory, the folder that holds `launcher-v2.sqlite` and `mod/`. The launcher database is opened read-only and immutable.

## Mod loading rules

- **Layer order:** vanilla, then the playset mods in launcher position order, then `--mod` arguments.
- **Files:** a file at the same relative path in a later layer replaces the earlier one. `replace_path="common/x"` in a mod descriptor hides the earlier layers' files under `common/x`.
- **Load order inside a folder:** files merged from all layers load in file-name order. When a tech key is defined more than once, the definition loaded last wins, and `source` names the layer it came from.
- **Localisation:** every `*_l_<lang>.yml` from every layer is loaded in layer order. Files under any `replace/` folder are applied last.
- **`inline_script`:** inline scripts are expanded, including parameters, `$P|default$` and computed script names such as `case_@[ x ]`.
- **Unknown prerequisites:** a prerequisite that doesn't resolve, for example a cross-mod reference, is reported in `meta.warnings` and doesn't stop the run.

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
    "cost_modifiers": [],                 // omitted when empty; same shape as weight_modifiers
    "cost_per_level": null, "levels": null, // repeatables only
    "base_weight": 95, "weight_factor": null,
    "is_start_tech": false, "is_rare": false, "is_dangerous": false, "is_repeatable": false,
    "is_insight": false, "is_reverse_engineerable": null,
    "is_event": false,                    // never drawn randomly (weight 0 / factor 0 / always-0 modifier)
    "gateway": null,
    "prerequisites": ["tech_lasers_1"],
    "unlocks": [{ "kind": "component_templates", "key": "SMALL_BLUE_LASER", "name": "Small Blue Laser" }],
    "requires": { "is_gestalt": false },  // top-level yes/no conditions of `potential`
    "potential": Trigger | null,
    "starting_potential": Trigger,        // omitted when absent
    "weight_modifiers": [{ "factor": 1.25, "add": null, "condition": Trigger }],
    "swaps": [{ "key", "name", "trigger": Trigger }],  // technology_swap variants, omitted when empty
    "source": "vanilla",                  // or the mod name
    "file": "common/technology/00_phys_weapon_tech.txt"
  }]
}
```

The values of `unlocks[].kind` come from these sources:

| Kind | Source |
|---|---|
| `modifier` | the tech's `modifier = {}` |
| `tooltip` | `custom_tooltip` |
| `feature` | `feature_flags` |
| `prereqfor_desc` kind (`ship`, `custom`, ...) | `prereqfor_desc` |
| `common/` folder name (`component_templates`, `buildings`, ...) | any object whose `prerequisites`, or whose `show_in_tech`, names the tech |

A `Trigger` node looks like this:

```jsonc
{ "key": "has_ethic", "op": "=", "value": "ethic_militarist", "text": "Has ethic: Militarist",
  "children": [ ... ],      // for blocks (AND/OR/NOT/scopes); omitted when empty
  "scripted": [ ... ] }     // scripted trigger body, expanded one level; omitted otherwise
```

`AND`, `OR`, `NOT`, `NOR` and `NAND` get generic wording. Other keys use the localisation key `trigger_<key>` if it exists, otherwise the key is humanized (`has_ethic` → "Has ethic"). Values are localised when a matching key exists, so `has_technology = tech_x` renders with the tech's name.

## Development

```sh
cargo test                    # unit + fixture integration tests (tests/fixtures: mini game + two mods)
cargo test -- --ignored       # acceptance against the real install (set STELLARIS_GAME to override the path)
cargo clippy --all-targets -- -D warnings
```

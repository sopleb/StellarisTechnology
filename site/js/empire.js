// "My empire": evaluate tech conditions against the player's empire setup.
// Each condition comes out true, false, or null ("depends on your game": flags, other techs,
// anything the setup doesn't pin down). Nothing about the game is hard-coded: the picker lists
// the values the dataset's conditions actually test.
(function () {
  "use strict";
  var ST = window.ST;

  // Picker fields: the condition keys each one answers.
  var FIELDS = {
    origin: ["has_origin"],
    civics: ["has_civic", "has_valid_civic"],
    ethics: ["has_ethic"],
    authority: ["has_authority"],
    archetype: ["is_archetype"],
    ships: ["uses_ship_category"],
    dlcs: ["host_has_dlc", "has_dlc"],
  };

  ST.emptyProfile = function () {
    return { origin: null, civics: null, ethics: null, authority: null, archetype: null, ships: null, nomadic: null, dlcs: null };
  };

  ST.profileIsEmpty = function (p) {
    return Object.keys(p).every(function (k) { return p[k] == null; });
  };

  function eachTrigger(model, fn) {
    function walk(t) {
      fn(t);
      (t.children || []).forEach(walk);
      (t.scripted || []).forEach(walk);
    }
    model.techs.forEach(function (t) {
      [t.potential, t.starting_potential].forEach(function (tr) { if (tr) walk(tr); });
    });
  }

  // {field: [{value, name}]} for the values this dataset's conditions test, sorted by name.
  ST.empireOptions = function (model) {
    if (model.empireOptions) return model.empireOptions;
    var byKey = {};
    Object.keys(FIELDS).forEach(function (f) { FIELDS[f].forEach(function (k) { byKey[k] = f; }); });
    var found = {};
    Object.keys(FIELDS).forEach(function (f) { found[f] = new Map(); });
    var nomadic = false;
    eachTrigger(model, function (t) {
      if (t.key === "is_nomadic") nomadic = true;
      var f = byKey[t.key];
      if (!f || t.value == null || /[@$\[]|^event_target:/.test(t.value)) return;
      // DLC names are already readable ("Apocalypse"); keys like BIOLOGICAL get humanized.
      var name = t.value_name || (f === "dlcs" ? t.value : ST.humanize(t.value.toLowerCase()));
      if (!found[f].has(t.value)) found[f].set(t.value, name);
    });
    var out = { nomadic: nomadic };
    Object.keys(found).forEach(function (f) {
      var list = Array.from(found[f], function (e) { return { value: e[0], name: e[1] }; });
      // Two values with one name (hive and machine variants of a civic): add the key to tell them apart.
      var seen = {};
      list.forEach(function (o) { seen[o.name] = (seen[o.name] || 0) + 1; });
      list.forEach(function (o) {
        if (seen[o.name] > 1) o.name += " (" + o.value.replace(/^(civic|origin|ethic|auth)_/, "") + ")";
      });
      list.sort(function (a, b) { return a.name.localeCompare(b.name); });
      out[f] = list;
    });
    model.empireOptions = out;
    return out;
  };

  // ---- three-valued logic ----------------------------------------------------
  function and(xs) {
    var unknown = false;
    for (var i = 0; i < xs.length; i++) {
      if (xs[i] === false) return false;
      if (xs[i] == null) unknown = true;
    }
    return unknown ? null : true;
  }
  function or(xs) {
    var unknown = false;
    for (var i = 0; i < xs.length; i++) {
      if (xs[i] === true) return true;
      if (xs[i] == null) unknown = true;
    }
    return unknown ? null : false;
  }
  function not(x) { return x == null ? null : !x; }
  function yesno(v, b) {
    if (b == null) return null;
    return v === "yes" ? b : v === "no" ? !b : null;
  }
  // "*" stands for every option (all DLC), which keeps shared links short.
  function has(list, v) { return list == null ? null : list === "*" || list.indexOf(v) >= 0; }
  function is(x, v) { return x == null ? null : x === v; }

  function gestalt(p, kind) {
    if (p.authority == null) return null;
    var a = p.authority;
    if (kind === "hive") return /hive/.test(a);
    if (kind === "machine") return /machine/.test(a);
    return /hive|machine/.test(a);
  }

  // Truth of `key = value` for keys the setup answers; undefined for keys it doesn't.
  function base(k, v, p) {
    switch (k) {
      case "host_has_dlc": case "has_dlc": return has(p.dlcs, v);
      case "has_origin": return is(p.origin, v);
      case "has_civic": case "has_valid_civic": return has(p.civics, v);
      case "has_ethic": return has(p.ethics, v);
      case "has_authority": return is(p.authority, v);
      case "is_archetype": return is(p.archetype, v);
      case "uses_ship_category": return p.ships == null ? null : p.ships === v;
      case "is_nomadic": return yesno(v, p.nomadic);
      case "is_gestalt": return yesno(v, gestalt(p));
      case "is_country_type": return v === "default";
      case "is_primitive": return yesno(v, false);
      case "always": return v === "yes" ? true : v === "no" ? false : null;
      case "exists": return v === "this" ? true : null;
    }
    return undefined;
  }

  function ev(t, p) {
    var k = t.key, kids = t.children || [];
    var r;
    if (k === "AND") r = and(kids.map(function (c) { return ev(c, p); }));
    else if (k === "OR") r = or(kids.map(function (c) { return ev(c, p); }));
    else if (k === "NOT" || k === "NOR") r = not(or(kids.map(function (c) { return ev(c, p); })));
    else if (k === "NAND") r = not(and(kids.map(function (c) { return ev(c, p); })));
    else if (kids.length) {
      // Scopes: the founder species is the player's choice; other scopes depend on the game.
      r = k === "founder_species" ? and(kids.map(function (c) { return ev(c, p); })) : null;
    } else {
      r = base(k, t.value, p);
      // Scripted triggers: the expanded body says when `key = yes` holds.
      if (r === undefined && t.scripted && t.scripted.length) {
        r = yesno(t.value, and(t.scripted.map(function (c) { return ev(c, p); })));
      }
      if (r === undefined && k === "is_hive_empire") r = yesno(t.value, gestalt(p, "hive"));
      if (r === undefined && k === "is_machine_empire") r = yesno(t.value, gestalt(p, "machine"));
      if (r === undefined) r = null;
    }
    if (t.op === "!=" || t.op === "<>") r = not(r);
    return r;
  }

  ST.evalTrigger = function (t, p) { return t ? ev(t, p) : true; };

  // {avail, start}: can this empire ever have the tech, and does it start with it?
  ST.computeStatus = function (model, p) {
    var map = new Map();
    model.techs.forEach(function (t) {
      var avail = ST.evalTrigger(t.potential, p);
      var start = t.is_start_tech ? and([avail, ST.evalTrigger(t.starting_potential, p)]) : false;
      map.set(t.key, { avail: avail, start: start });
    });
    return map;
  };

  // ---- serialisation for the URL and browser storage ---------------------------
  var SHORT = { origin: "o", civics: "c", ethics: "e", authority: "a", archetype: "s", ships: "b", nomadic: "n", dlcs: "d" };
  ST.encodeProfile = function (p) {
    var o = {};
    Object.keys(SHORT).forEach(function (k) { if (p[k] != null) o[SHORT[k]] = p[k]; });
    return Object.keys(o).length ? JSON.stringify(o) : "";
  };
  ST.decodeProfile = function (s) {
    var p = ST.emptyProfile();
    if (!s) return p;
    try {
      var o = JSON.parse(s);
      Object.keys(SHORT).forEach(function (k) { if (o[SHORT[k]] !== undefined) p[k] = o[SHORT[k]]; });
    } catch (e) { /* ignore a malformed link */ }
    return p;
  };
})();

// Indexes over a techs.json document.
(function () {
  "use strict";
  var ST = window.ST;

  ST.prepare = function (raw) {
    if (!raw || !Array.isArray(raw.techs)) throw new Error("not a stellaris-tech techs.json (no techs array)");
    var techs = raw.techs;
    var byKey = new Map();
    techs.forEach(function (t) { byKey.set(t.key, t); });

    var dependents = new Map();
    techs.forEach(function (t) { dependents.set(t.key, []); });
    techs.forEach(function (t) {
      ST.prereqKeys(t).forEach(function (p) {
        if (dependents.has(p)) dependents.get(p).push(t.key);
      });
    });

    var areas = (raw.areas || []).slice();
    var seen = new Set(areas.map(function (a) { return a.key; }));
    techs.forEach(function (t) {
      if (!seen.has(t.area)) {
        seen.add(t.area);
        areas.push({ key: t.area, name: ST.humanize(t.area) });
      }
    });

    var catName = new Map();
    var catArea = new Map();
    (raw.categories || []).forEach(function (c) {
      catName.set(c.key, c.name);
      if (c.area) catArea.set(c.key, c.area);
    });
    var catOrder = (raw.categories || []).map(function (c) { return c.key; });
    techs.forEach(function (t) {
      (t.category || []).forEach(function (c) {
        if (!catName.has(c)) {
          catName.set(c, ST.humanize(c));
          catOrder.push(c);
        }
        if (!catArea.has(c)) catArea.set(c, t.area);
      });
    });

    var tierKeys = (raw.tiers || []).map(function (t) { return t.key; });
    techs.forEach(function (t) {
      if (t.tier != null && tierKeys.indexOf(t.tier) < 0) tierKeys.push(t.tier);
    });

    var sources = [];
    techs.forEach(function (t) { if (sources.indexOf(t.source) < 0) sources.push(t.source); });

    // Lower-cased haystack for search: name, key, category names and unlock names.
    var hay = new Map();
    techs.forEach(function (t) {
      var parts = [t.name, t.key];
      (t.category || []).forEach(function (c) { parts.push(catName.get(c)); });
      (t.unlocks || []).forEach(function (u) { parts.push(u.name, u.key); });
      hay.set(t.key, parts.join("\n").toLowerCase());
    });

    return {
      raw: raw,
      meta: raw.meta || {},
      techs: techs,
      byKey: byKey,
      dependents: dependents,
      areas: areas,
      catName: catName,
      catArea: catArea,
      catOrder: catOrder,
      tierKeys: tierKeys,
      sources: sources,
      hay: hay,
      layouts: new Map(),
    };
  };

  function walk(model, start, next) {
    var out = new Set();
    var stack = next(start).slice();
    while (stack.length) {
      var k = stack.pop();
      if (out.has(k) || k === start) continue;
      out.add(k);
      next(k).forEach(function (n) { stack.push(n); });
    }
    return out;
  }

  // Every tech needed before `key` (transitive prerequisites that exist in the dataset).
  ST.ancestors = function (model, key) {
    return walk(model, key, function (k) {
      var t = model.byKey.get(k);
      return t ? ST.prereqKeys(t).filter(function (p) { return model.byKey.has(p); }) : [];
    });
  };

  // Every tech a prerequisite links to: required ones plus all "one of" alternatives.
  ST.prereqKeys = function (t) {
    var out = (t.prerequisites || []).slice();
    (t.prerequisite_groups || []).forEach(function (g) {
      g.forEach(function (k) { if (out.indexOf(k) < 0) out.push(k); });
    });
    return out;
  };

  // The cheapest set of techs needed before `key`: required prerequisites, plus the
  // cheapest option of each "one of" group, recursively.
  ST.cheapestChain = function (model, key) {
    var memo = new Map();
    function cost(set) {
      var c = 0;
      set.forEach(function (k) {
        var t = model.byKey.get(k);
        if (t && t.cost && !t.is_start_tech) c += t.cost;
      });
      return c;
    }
    function chain(k, stack) {
      if (memo.has(k)) return memo.get(k);
      var t = model.byKey.get(k), out = new Set();
      if (!t || stack.has(k)) return out;
      stack.add(k);
      function take(p) {
        if (!model.byKey.has(p)) return;
        out.add(p);
        chain(p, stack).forEach(function (x) { out.add(x); });
      }
      (t.prerequisites || []).forEach(take);
      (t.prerequisite_groups || []).forEach(function (g) {
        var best = null, bestCost = Infinity;
        g.forEach(function (p) {
          if (!model.byKey.has(p)) return;
          var s = new Set(chain(p, stack));
          s.add(p);
          var c = cost(s);
          if (c < bestCost) { best = p; bestCost = c; }
        });
        if (best) take(best);
      });
      stack.delete(k);
      memo.set(k, out);
      return out;
    }
    var set = chain(key, new Set());
    return { techs: set, cost: cost(set) };
  };

  ST.descendants = function (model, key) {
    return walk(model, key, function (k) { return model.dependents.get(k) || []; });
  };

  ST.techName = function (model, key) {
    var t = model.byKey.get(key);
    return t ? t.name : key;
  };

  // The filter state is {q, area, event, repeatable, only: [flag], tiers: Set, cats: Set, source}.
  ST.matcher = function (model, f) {
    var q = (f.q || "").trim().toLowerCase();
    var terms = q ? q.split(/\s+/) : [];
    return function (t) {
      if (!f.event && t.is_event) return false;
      if (!f.repeatable && t.is_repeatable) return false;
      for (var i = 0; i < f.only.length; i++) if (!t[f.only[i]]) return false;
      if (f.tiers.size && !f.tiers.has(t.tier)) return false;
      if (f.cats.size && !(t.category || []).some(function (c) { return f.cats.has(c); })) return false;
      if (f.source && t.source !== f.source) return false;
      if (terms.length) {
        var h = model.hay.get(t.key);
        for (var j = 0; j < terms.length; j++) if (h.indexOf(terms[j]) < 0) return false;
      }
      return true;
    };
  };

  ST.flags = function (t) {
    var out = [];
    if (t.is_start_tech) out.push(["start", "Starting"]);
    if (t.is_rare) out.push(["rare", "Rare"]);
    if (t.is_dangerous) out.push(["dangerous", "Dangerous"]);
    if (t.is_repeatable) out.push(["repeatable", "Repeatable"]);
    if (t.is_event) out.push(["event", "Event"]);
    if (t.is_insight) out.push(["insight", "Insight"]);
    return out;
  };
})();

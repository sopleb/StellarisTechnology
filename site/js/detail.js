// The side panel describing one technology.
(function () {
  "use strict";
  var ST = window.ST, el = ST.el;

  function techLink(model, key, onSelect) {
    var t = model.byKey.get(key);
    if (!t) return el("span", { class: "chip missing", title: "This dataset doesn't define this tech" }, key);
    return el(
      "a",
      {
        class: "chip tech-link",
        href: "#",
        style: { "--area": ST.areaColor(t.area) },
        title: t.key,
        onclick: function (e) { e.preventDefault(); onSelect(key, true); },
      },
      t.name
    );
  }

  function trigger(t) {
    var li = el(
      "li",
      null,
      el("span", { class: "tr-text", title: t.key + " " + t.op + (t.value != null ? " " + t.value : "") }, t.text)
    );
    if (t.children && t.children.length) li.appendChild(el("ul", null, t.children.map(trigger)));
    if (t.scripted && t.scripted.length) {
      li.appendChild(
        el("details", { class: "scripted" }, el("summary", null, "definition"), el("ul", null, t.scripted.map(trigger)))
      );
    }
    return li;
  }

  function triggerTree(t) {
    return el("ul", { class: "trigger" }, trigger(t));
  }

  function section(title, body, extra) {
    return el("section", { class: "d-sec" }, el("h3", null, title, extra || null), body);
  }

  function modifiers(list, base) {
    return el(
      "ol",
      { class: "mods" },
      list.map(function (m) {
        var badge = [];
        if (m.factor != null) badge.push(el("span", { class: "mult" + (m.factor === 0 ? " zero" : m.factor < 1 ? " down" : " up") }, "×" + ST.fmtNum(m.factor)));
        if (m.add != null) badge.push(el("span", { class: "mult" + (m.add < 0 ? " down" : " up") }, (m.add >= 0 ? "+" : "") + ST.fmtNum(m.add)));
        return el(
          "li",
          null,
          el("div", { class: "mod-val" }, badge),
          m.condition ? triggerTree(m.condition) : el("div", { class: "muted" }, base || "Always")
        );
      })
    );
  }

  function unlocks(model, list) {
    var groups = new Map();
    list.forEach(function (u) {
      var k = ST.kindLabel(u.kind);
      if (!groups.has(k)) groups.set(k, []);
      groups.get(k).push(u);
    });
    return el(
      "div",
      { class: "unlocks" },
      Array.from(groups.keys()).map(function (k) {
        return el(
          "div",
          { class: "ugroup" },
          el("div", { class: "ukind" }, k),
          el(
            "ul",
            null,
            groups.get(k).map(function (u) {
              return el("li", { title: u.kind + ": " + u.key }, u.name);
            })
          )
        );
      })
    );
  }

  ST.renderDetail = function (root, model, key, onSelect, onClose) {
    ST.clear(root);
    var t = model.byKey.get(key);
    if (!t) {
      root.hidden = true;
      return;
    }
    root.hidden = false;
    root.style.setProperty("--area", ST.areaColor(t.area));

    var areaName = (model.areas.find(function (a) { return a.key === t.area; }) || {}).name || t.area;
    var cats = (t.category || []).map(function (c) { return model.catName.get(c) || c; }).join(", ");

    var head = el(
      "header",
      { class: "d-head" },
      el("button", { class: "icon close", title: "Close (Esc)", "aria-label": "Close", onclick: onClose }, "✕"),
      el("div", { class: "d-kicker" }, areaName, cats ? " · " + cats : "", t.tier != null ? " · Tier " + t.tier : ""),
      el("h2", null, t.name),
      el("code", { class: "d-key" }, t.key),
      el(
        "div",
        { class: "flags" },
        ST.flags(t).map(function (f) { return el("span", { class: "flag " + f[0], title: f[2] }, f[1]); }),
        t.gateway ? el("span", { class: "flag gateway", title: "Gateway technology" }, "Gateway: " + ST.humanize(t.gateway)) : null
      )
    );

    var stats = el(
      "dl",
      { class: "stats" },
      el("div", null, el("dt", null, "Cost"), el("dd", null, ST.fmtNum(t.cost), t.cost_expr && t.cost_expr !== String(t.cost) ? el("small", null, t.cost_expr) : null)),
      t.is_repeatable && t.cost_per_level != null
        ? el("div", null, el("dt", null, "Per level"), el("dd", null, "+" + ST.fmtNum(t.cost_per_level)))
        : null,
      t.is_repeatable ? el("div", null, el("dt", null, "Levels"), el("dd", null, t.levels == null || t.levels < 0 ? "∞" : ST.fmtNum(t.levels))) : null,
      el("div", null, el("dt", null, "Weight"), el("dd", null, ST.fmtNum(t.base_weight), t.weight_factor != null ? el("small", null, "× " + ST.fmtNum(t.weight_factor)) : null))
    );

    var path = ST.cheapestChain(model, t.key);
    var groups = t.prerequisite_groups || [];
    var direct = (t.prerequisites || []).length + groups.length;
    var deps = model.dependents.get(t.key) || [];

    var body = [
      t.description ? el("p", { class: "d-desc" }, t.description) : null,
      stats,
      t.is_start_tech && !ST.startsForAll(t)
        ? section(
            "Starting tech only when",
            el("div", null, ST.startConditions(t).map(triggerTree)),
            t.starting_potential && t.potential ? el("span", { class: "h-note" }, "all of these") : null
          )
        : null,
      section(
        "Requires",
        direct
          ? el(
              "div",
              { class: "prereqs" },
              (t.prerequisites || []).length
                ? el("div", { class: "chips" }, t.prerequisites.map(function (k) { return techLink(model, k, onSelect); }))
                : null,
              groups.map(function (g) {
                return el(
                  "div",
                  { class: "one-of" },
                  el("span", { class: "one-of-label" }, (t.prerequisites || []).length ? "and one of" : "One of"),
                  el("div", { class: "chips" }, g.map(function (k) { return techLink(model, k, onSelect); }))
                );
              })
            )
          : el("div", { class: "muted" }, "No prerequisites"),
        path.techs.size > direct
          ? el(
              "span",
              { class: "h-note", title: groups.length ? "Counts the cheapest option of each “one of” choice" : null },
              path.techs.size + " techs in the full chain · " + ST.fmtNum(path.cost) + " research" + (groups.length ? " (cheapest)" : "")
            )
          : null
      ),
      deps.length
        ? section("Leads to", el("div", { class: "chips" }, deps.map(function (k) { return techLink(model, k, onSelect); })))
        : null,
      t.unlocks && t.unlocks.length ? section("Unlocks", unlocks(model, t.unlocks)) : null,
      t.requires && Object.keys(t.requires).length
        ? section(
            "Empire",
            el(
              "div",
              { class: "chips" },
              Object.keys(t.requires).map(function (k) {
                return el("span", { class: "chip req " + (t.requires[k] ? "yes" : "no") }, (t.requires[k] ? "" : "Not ") + ST.humanize(k).toLowerCase().replace(/^./, function (c) { return c.toUpperCase(); }));
              })
            )
          )
        : null,
      t.potential ? section("Available when", triggerTree(t.potential)) : null,
      t.weight_modifiers && t.weight_modifiers.length ? section("Weight modifiers", modifiers(t.weight_modifiers)) : null,
      t.cost_modifiers && t.cost_modifiers.length ? section("Cost modifiers", modifiers(t.cost_modifiers)) : null,
      t.swaps && t.swaps.length
        ? section(
            "Variants",
            el(
              "ul",
              { class: "swaps" },
              t.swaps.map(function (s) {
                return el("li", null, el("strong", null, s.name), el("code", null, " " + s.key), s.trigger ? triggerTree(s.trigger) : null);
              })
            )
          )
        : null,
      el("footer", { class: "d-src" }, el("span", null, t.source), el("code", null, t.file)),
    ];

    root.appendChild(head);
    root.appendChild(el("div", { class: "d-body" }, body));
    root.scrollTop = 0;
  };
})();

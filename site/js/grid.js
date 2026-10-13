// Card view: techs grouped by area, then tier (repeatables last).
(function () {
  "use strict";
  var ST = window.ST, el = ST.el;

  function card(model, t, onSelect) {
    var cats = (t.category || []).map(function (c) { return model.catName.get(c) || c; }).join(", ");
    var pre = (t.prerequisites || []).map(function (k) { return ST.techName(model, k); });
    (t.prerequisite_groups || []).forEach(function (g) {
      pre.push(g.map(function (k) { return ST.techName(model, k); }).join(" or "));
    });
    return el(
      "button",
      {
        class: "card" + (t.is_rare ? " rare" : "") + (t.is_dangerous ? " dangerous" : "") + (t.is_event ? " event" : ""),
        "data-key": t.key,
        style: { "--area": ST.areaColor(t.area) },
        onclick: function () { onSelect(t.key, false); },
      },
      el("span", { class: "c-name" }, t.name),
      el("span", { class: "c-sub" }, cats, el("span", { class: "c-cost" }, ST.fmtNum(t.cost))),
      ST.flags(t).length
        ? el("span", { class: "c-flags" }, ST.flags(t).map(function (f) { return el("span", { class: "flag " + f[0] }, f[1]); }))
        : null,
      pre.length ? el("span", { class: "c-pre", title: pre.join(", ") }, "← " + pre.join(", ")) : null
    );
  }

  function byCatThenName(model) {
    return function (a, b) {
      var ca = model.catOrder.indexOf((a.category || [])[0]);
      var cb = model.catOrder.indexOf((b.category || [])[0]);
      return ca - cb || a.name.localeCompare(b.name);
    };
  }

  ST.renderGrid = function (root, model, area, match, onSelect) {
    ST.clear(root);
    var areas = area === "all" ? model.areas : model.areas.filter(function (a) { return a.key === area; });
    var shown = 0;
    var sort = byCatThenName(model);
    var wrap = el("div", { class: "grid-view" });
    areas.forEach(function (a) {
      var list = model.techs.filter(function (t) { return t.area === a.key && match(t); });
      if (!list.length) return;
      shown += list.length;
      var groups = model.tierKeys
        .map(function (k) {
          return { title: "Tier " + k, items: list.filter(function (t) { return t.tier === k && !t.is_repeatable; }) };
        })
        .concat([
          { title: "No tier", items: list.filter(function (t) { return t.tier == null && !t.is_repeatable; }) },
          { title: "Repeatable", items: list.filter(function (t) { return t.is_repeatable; }) },
        ])
        .filter(function (g) { return g.items.length; });
      wrap.appendChild(
        el(
          "section",
          { class: "g-area", style: { "--area": ST.areaColor(a.key) } },
          el("h2", null, a.name, el("span", { class: "count" }, list.length)),
          groups.map(function (g) {
            return el(
              "div",
              { class: "g-tier" },
              el("h3", null, g.title, el("span", { class: "count" }, g.items.length)),
              el("div", { class: "cards" }, g.items.sort(sort).map(function (t) { return card(model, t, onSelect); }))
            );
          })
        )
      );
    });
    if (!shown) wrap.appendChild(el("div", { class: "status" }, "No technologies match the current search and filters."));
    root.appendChild(wrap);
    return {
      shown: shown,
      select: function (key, scroll) {
        Array.prototype.forEach.call(root.querySelectorAll(".card.sel"), function (c) { c.classList.remove("sel"); });
        if (!key) return;
        var c = root.querySelector('.card[data-key="' + CSS.escape(key) + '"]');
        if (c) {
          c.classList.add("sel");
          if (scroll) c.scrollIntoView({ block: "center", behavior: "smooth" });
        }
      },
    };
  };
})();

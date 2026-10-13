// App state, URL hash routing and wiring.
(function () {
  "use strict";
  var ST = window.ST, el = ST.el, $ = ST.$;

  var state = {
    datasets: [],
    ds: null,
    model: null,
    view: ST.store.get("view", "grid"),
    area: "all",
    tech: null,
    filters: freshFilters(),
  };
  var current = null; // the rendered view controller
  var loadSeq = 0;

  function freshFilters() {
    return { q: "", event: true, repeatable: true, only: [], tiers: new Set(), cats: new Set(), source: "" };
  }

  // ---- hash: #ds=vanilla&view=graph&area=physics&tech=tech_lasers_2&q=laser ----
  function readHash() {
    var p = new URLSearchParams(location.hash.replace(/^#/, ""));
    return { ds: p.get("ds"), view: p.get("view"), area: p.get("area"), tech: p.get("tech"), q: p.get("q") };
  }
  function writeHash(push) {
    var p = new URLSearchParams();
    if (state.ds) p.set("ds", state.ds);
    p.set("view", state.view);
    if (state.area !== "all") p.set("area", state.area);
    if (state.tech) p.set("tech", state.tech);
    if (state.filters.q) p.set("q", state.filters.q);
    var h = "#" + p.toString();
    if (h === location.hash) return;
    if (push) history.pushState(null, "", h);
    else history.replaceState(null, "", h);
  }

  function status(msg, isError) {
    var c = ST.clear($("#content"));
    c.appendChild(el("div", { class: "status" + (isError ? " error" : "") }, msg));
  }

  // ---- data -----------------------------------------------------------------
  function loadIndex() {
    return fetch("data/datasets.json", { cache: "no-cache" })
      .then(function (r) {
        if (!r.ok) throw new Error("HTTP " + r.status);
        return r.json();
      })
      .catch(function () {
        return [];
      });
  }

  function loadDataset(id) {
    var d = state.datasets.find(function (x) { return x.id === id; }) || state.datasets[0];
    if (!d) {
      status("This site has no datasets yet. Use “Open file…” to load a techs.json from stellaris-tech.");
      return Promise.resolve();
    }
    var seq = ++loadSeq;
    status("Loading " + d.label + "…");
    return fetch("data/" + d.file)
      .then(function (r) {
        if (!r.ok) throw new Error("HTTP " + r.status + " for data/" + d.file);
        return r.json();
      })
      .then(function (raw) {
        if (seq !== loadSeq) return;
        state.ds = d.id;
        $("#dataset").value = d.id;
        setModel(ST.prepare(raw));
      })
      .catch(function (e) {
        if (seq !== loadSeq) return;
        status(
          "Could not load " + d.label + ": " + e.message +
            (location.protocol === "file:" ? ". Your browser blocks data loads on file:// pages. Serve the folder over HTTP, or use “Open file…”." : ""),
          true
        );
      });
  }

  function setModel(model) {
    state.model = model;
    if (state.area !== "all" && !model.areas.some(function (a) { return a.key === state.area; })) state.area = "all";
    if (state.tech && !model.byKey.has(state.tech)) state.tech = null;
    var keepQ = state.filters.q;
    state.filters = freshFilters();
    state.filters.q = keepQ;
    buildChrome();
    render();
  }

  // ---- chrome (tabs, filter options, footer) ---------------------------------
  function buildChrome() {
    var m = state.model;
    var tabs = ST.clear($("#areas"));
    [{ key: "all", name: "All" }].concat(m.areas).forEach(function (a) {
      var n = a.key === "all" ? m.techs.length : m.techs.filter(function (t) { return t.area === a.key; }).length;
      tabs.appendChild(
        el(
          "button",
          {
            class: "tab" + (state.area === a.key ? " on" : ""),
            "data-area": a.key,
            style: a.key === "all" ? null : { "--area": ST.areaColor(a.key) },
            onclick: function () { setArea(a.key); },
          },
          a.name,
          el("span", { class: "count" }, n)
        )
      );
    });

    var tiers = ST.clear($("#tiers"));
    m.tierKeys.forEach(function (k) {
      tiers.appendChild(chipToggle(k, "T" + k, state.filters.tiers));
    });
    buildCats();

    var srcFs = $("#src-fs"), src = $("#source");
    srcFs.hidden = m.sources.length < 2;
    ST.clear(src).appendChild(el("option", { value: "" }, "All sources"));
    m.sources.forEach(function (s) { src.appendChild(el("option", { value: s }, s)); });

    var meta = m.meta, foot = ST.clear($("#meta"));
    var bits = [
      el("span", null, "Stellaris " + (meta.game_version || "?")),
      el("span", null, (meta.tech_count || m.techs.length) + " techs"),
      meta.language ? el("span", null, meta.language) : null,
      meta.playset ? el("span", null, "Playset: " + meta.playset) : null,
    ];
    if (meta.mods && meta.mods.length) {
      bits.push(
        el(
          "details",
          { class: "inline" },
          el("summary", null, meta.mods.length + " mod" + (meta.mods.length > 1 ? "s" : "")),
          el("ol", null, meta.mods.map(function (x) {
            return el("li", null, x.remote_id ? el("a", { href: "https://steamcommunity.com/sharedfiles/filedetails/?id=" + x.remote_id, target: "_blank", rel: "noopener" }, x.name) : x.name);
          }))
        )
      );
    }
    if (meta.warnings && meta.warnings.length) {
      bits.push(
        el("details", { class: "inline warn" }, el("summary", null, meta.warnings.length + " warning" + (meta.warnings.length > 1 ? "s" : "")),
          el("ul", null, meta.warnings.map(function (w) { return el("li", null, w); })))
      );
    }
    bits.push(el("span", { class: "gen" }, meta.generator || ""));
    bits.push(el("a", { href: "https://github.com/sopleb/stellaris-tech-rs", target: "_blank", rel: "noopener" }, "Source"));
    foot.appendChild(el("div", { class: "meta-row" }, bits));
  }

  function buildCats() {
    var m = state.model, cats = ST.clear($("#cats"));
    m.catOrder
      .filter(function (c) { return state.area === "all" || m.catArea.get(c) === state.area; })
      .filter(function (c) { return m.techs.some(function (t) { return (t.category || []).indexOf(c) >= 0; }); })
      .forEach(function (c) {
        cats.appendChild(chipToggle(c, m.catName.get(c), state.filters.cats, ST.areaColor(m.catArea.get(c) || "")));
      });
  }

  function chipToggle(value, label, set, color) {
    return el(
      "button",
      {
        class: "chip toggle" + (set.has(value) ? " on" : ""),
        style: color ? { "--area": color } : null,
        "aria-pressed": set.has(value) ? "true" : "false",
        onclick: function (e) {
          if (set.has(value)) set.delete(value);
          else set.add(value);
          e.currentTarget.classList.toggle("on", set.has(value));
          e.currentTarget.setAttribute("aria-pressed", set.has(value) ? "true" : "false");
          refilter();
        },
      },
      label
    );
  }

  // ---- render ----------------------------------------------------------------
  function render() {
    if (!state.model) return;
    Array.prototype.forEach.call(document.querySelectorAll("#areas .tab"), function (b) {
      b.classList.toggle("on", b.getAttribute("data-area") === state.area);
    });
    Array.prototype.forEach.call(document.querySelectorAll(".seg button"), function (b) {
      var on = b.getAttribute("data-view") === state.view;
      b.classList.toggle("on", on);
      b.setAttribute("aria-selected", on ? "true" : "false");
    });
    var content = $("#content");
    var match = ST.matcher(state.model, state.filters);
    if (state.view === "graph") {
      current = ST.renderGraph(content, state.model, state.area, select);
      current.setMatch(match);
      current.select(state.tech, true);
    } else {
      current = ST.renderGrid(content, state.model, state.area, match, select);
      current.select(state.tech, true);
    }
    updateCounts(match);
    renderDetail();
    writeHash(false);
  }

  function refilter() {
    if (!state.model) return;
    var match = ST.matcher(state.model, state.filters);
    if (state.view === "graph" && current && current.setMatch) current.setMatch(match);
    else render();
    updateCounts(match);
    writeHash(false);
  }

  function updateCounts(match) {
    var m = state.model, f = state.filters;
    var inArea = m.techs.filter(function (t) { return state.area === "all" || t.area === state.area; });
    var n = inArea.filter(match).length;
    $("#hits").textContent = f.q || activeFilterCount() ? n + " / " + inArea.length : "";
    var c = activeFilterCount();
    $("#filter-count").textContent = c ? String(c) : "";
  }

  function activeFilterCount() {
    var f = state.filters;
    return (f.event ? 0 : 1) + (f.repeatable ? 0 : 1) + f.only.length + f.tiers.size + f.cats.size + (f.source ? 1 : 0);
  }

  function renderDetail() {
    ST.renderDetail($("#detail"), state.model, state.tech, select, function () { select(null); });
    document.body.classList.toggle("has-detail", !!state.tech);
  }

  // ---- actions -------------------------------------------------------------
  function select(key, fromLink) {
    var t = key && state.model.byKey.get(key);
    // Following a link to a tech in another area switches area so it stays visible.
    if (t && state.area !== "all" && t.area !== state.area) {
      state.tech = key;
      state.area = t.area;
      writeHash(true);
      buildCats();
      render();
      return;
    }
    state.tech = key || null;
    if (current) current.select(state.tech, !!fromLink);
    renderDetail();
    writeHash(true);
  }

  function setArea(a) {
    if (a === state.area) return;
    state.area = a;
    buildCats();
    writeHash(true);
    render();
  }

  function setView(v) {
    if (v === state.view) return;
    state.view = v;
    ST.store.set("view", v);
    writeHash(true);
    render();
  }

  function applyHash() {
    var h = readHash();
    if (h.view === "graph" || h.view === "grid") state.view = h.view;
    state.area = h.area || "all";
    state.tech = h.tech || null;
    state.filters.q = h.q || "";
    $("#q").value = state.filters.q;
    if (h.ds && h.ds !== state.ds && state.datasets.some(function (d) { return d.id === h.ds; })) return loadDataset(h.ds);
    if (state.model) {
      buildChrome();
      render();
    }
  }

  // ---- wiring ----------------------------------------------------------------
  function wire() {
    $("#dataset").addEventListener("change", function (e) {
      state.tech = null;
      loadDataset(e.target.value).then(function () { writeHash(true); });
    });

    $("#file").addEventListener("change", function (e) {
      var f = e.target.files && e.target.files[0];
      if (!f) return;
      var r = new FileReader();
      r.onload = function () {
        try {
          var model = ST.prepare(JSON.parse(r.result));
          var sel = $("#dataset");
          var opt = sel.querySelector('option[value="local"]') || sel.appendChild(el("option", { value: "local" }));
          opt.textContent = "Local: " + f.name;
          sel.value = "local";
          state.ds = null;
          loadSeq++;
          setModel(model);
        } catch (err) {
          status("Could not read " + f.name + ": " + err.message, true);
        }
      };
      r.readAsText(f);
      e.target.value = "";
    });

    Array.prototype.forEach.call(document.querySelectorAll(".seg button"), function (b) {
      b.addEventListener("click", function () { setView(b.getAttribute("data-view")); });
    });

    var q = $("#q");
    q.addEventListener("input", ST.debounce(function () {
      state.filters.q = q.value;
      refilter();
    }, 120));
    q.addEventListener("keydown", function (e) {
      if (e.key === "Enter" && state.model) {
        state.filters.q = q.value;
        var match = ST.matcher(state.model, state.filters);
        var first = state.model.techs.find(function (t) { return (state.area === "all" || t.area === state.area) && match(t); });
        if (first) select(first.key, true);
      } else if (e.key === "Escape") {
        q.value = "";
        state.filters.q = "";
        refilter();
        q.blur();
      }
    });

    var fb = $("#filters-btn"), panel = $("#filters");
    fb.addEventListener("click", function () {
      panel.hidden = !panel.hidden;
      fb.setAttribute("aria-expanded", panel.hidden ? "false" : "true");
    });
    Array.prototype.forEach.call(panel.querySelectorAll("input[data-flag]"), function (i) {
      i.addEventListener("change", function () {
        state.filters[i.getAttribute("data-flag")] = i.checked;
        refilter();
      });
    });
    Array.prototype.forEach.call(panel.querySelectorAll("input[data-only]"), function (i) {
      i.addEventListener("change", function () {
        var k = i.getAttribute("data-only"), only = state.filters.only;
        if (i.checked && only.indexOf(k) < 0) only.push(k);
        if (!i.checked) only.splice(only.indexOf(k), 1);
        refilter();
      });
    });
    $("#source").addEventListener("change", function (e) {
      state.filters.source = e.target.value;
      refilter();
    });
    $("#filters-reset").addEventListener("click", function () {
      var q0 = state.filters.q;
      state.filters = freshFilters();
      state.filters.q = q0;
      Array.prototype.forEach.call(panel.querySelectorAll("input[type=checkbox]"), function (i) {
        i.checked = !!i.getAttribute("data-flag");
      });
      $("#source").value = "";
      buildChrome();
      refilter();
    });

    document.addEventListener("keydown", function (e) {
      var typing = /INPUT|SELECT|TEXTAREA/.test((e.target && e.target.tagName) || "");
      if (e.key === "/" && !typing) {
        e.preventDefault();
        q.focus();
        q.select();
      } else if (e.key === "Escape" && !typing && state.tech) {
        select(null);
      }
    });

    window.addEventListener("popstate", applyHash);
    window.addEventListener("hashchange", applyHash);
  }

  wire();
  loadIndex().then(function (list) {
    state.datasets = list;
    var sel = ST.clear($("#dataset"));
    list.forEach(function (d) { sel.appendChild(el("option", { value: d.id }, d.label)); });
    var h = readHash();
    if (h.view === "graph" || h.view === "grid") state.view = h.view;
    state.area = h.area || "all";
    state.tech = h.tech || null;
    state.filters.q = h.q || "";
    $("#q").value = state.filters.q;
    return loadDataset(h.ds || (list[0] && list[0].id));
  });
})();

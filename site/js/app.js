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
    profile: ST.decodeProfile(ST.store.get("empire", "")),
    showAll: false,
  };
  var current = null; // the rendered view controller
  var loadSeq = 0;

  function freshFilters() {
    return { q: "", event: true, repeatable: true, only: [], tiers: new Set(), cats: new Set(), source: "" };
  }

  // ---- hash: #ds=vanilla&view=graph&area=physics&tech=tech_lasers_2&q=laser ----
  function readHash() {
    var p = new URLSearchParams(location.hash.replace(/^#/, ""));
    return { ds: p.get("ds"), view: p.get("view"), area: p.get("area"), tech: p.get("tech"), q: p.get("q"), emp: p.get("emp") };
  }
  function writeHash(push) {
    var p = new URLSearchParams();
    if (state.ds) p.set("ds", state.ds);
    p.set("view", state.view);
    if (state.area !== "all") p.set("area", state.area);
    if (state.tech) p.set("tech", state.tech);
    if (state.filters.q) p.set("q", state.filters.q);
    var emp = ST.encodeProfile(state.profile);
    if (emp) p.set("emp", emp);
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
    applyProfile();
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
      var n = m.techs.filter(function (t) { return (a.key === "all" || t.area === a.key) && !ST.hidden(m, t); }).length;
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
    bits.push(el("a", { href: "https://github.com/sopleb/StellarisTechnology", target: "_blank", rel: "noopener" }, "Source"));
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
    if (h.emp != null && h.emp !== ST.encodeProfile(state.profile)) {
      state.profile = ST.decodeProfile(h.emp);
      if (state.model) applyProfile();
    }
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
      } else if (e.key === "Escape" && !typing && (!$("#empire").hidden || !$("#filters").hidden)) {
        $("#empire").hidden = true;
        $("#filters").hidden = true;
        $("#empire-btn").setAttribute("aria-expanded", "false");
        $("#filters-btn").setAttribute("aria-expanded", "false");
      } else if (e.key === "Escape" && !typing && state.tech) {
        select(null);
      }
    });

    window.addEventListener("popstate", applyHash);
    window.addEventListener("hashchange", applyHash);
  }

  // ---- my empire ---------------------------------------------------------------
  // Recompute every tech's status for the chosen empire and remember the choice.
  function applyProfile() {
    var m = state.model;
    if (!m) return;
    m.customEmpire = !ST.profileIsEmpty(state.profile);
    m.status = ST.computeStatus(m, state.profile);
    m.showAll = state.showAll;
    m.profileKey = ST.encodeProfile(state.profile) + (state.showAll ? "+all" : "");
    ST.store.set("empire", ST.encodeProfile(state.profile));
    var hidden = 0;
    m.status.forEach(function (s) { if (s.avail === false) hidden++; });
    $("#empire-note").textContent = m.customEmpire
      ? hidden + " techs your empire can't get are " + (state.showAll ? "shown." : "hidden.")
      : "";
    $("#empire-sum").textContent = summary();
    $("#empire-btn").classList.toggle("set", m.customEmpire);
    buildEmpireFields();
  }

  function summary() {
    var p = state.profile, o = state.model ? ST.empireOptions(state.model) : {};
    if (ST.profileIsEmpty(p)) return "Any";
    var name = function (field, v) {
      var hit = (o[field] || []).find(function (x) { return x.value === v; });
      return hit ? hit.name : null;
    };
    var bits = [];
    if (p.origin) bits.push(name("origin", p.origin) || "Other origin");
    if (p.authority) bits.push(name("authority", p.authority) || "Other authority");
    if (p.civics && p.civics.length) bits.push(p.civics.length + " civic" + (p.civics.length > 1 ? "s" : ""));
    return bits.length ? bits.join(" · ") : "Custom";
  }

  function profileChanged() {
    applyProfile();
    buildChrome();
    writeHash(false);
    render();
  }

  function buildEmpireFields() {
    var m = state.model, p = state.profile, box = ST.clear($("#empire-fields"));
    if (!m) return;
    var o = ST.empireOptions(m);

    function select(field, label, choices, other) {
      if (!choices.length && !other) return;
      var s = el(
        "select",
        {
          onchange: function (e) {
            var v = e.target.value;
            p[field] = v === "" ? null : v === "__no" ? false : v === "__yes" ? true : v === "__none" ? "" : v;
            profileChanged();
          },
        },
        el("option", { value: "" }, "Any"),
        choices.map(function (c) { return el("option", { value: c.value }, c.name); }),
        other ? el("option", { value: other.value }, other.name) : null
      );
      var cur = p[field];
      s.value = cur == null ? "" : cur === false ? "__no" : cur === true ? "__yes" : cur === "" ? "__none" : cur;
      box.appendChild(el("label", { class: "emp-field" }, el("span", null, label), s));
    }

    function chips(field, label, choices, extra) {
      if (!choices.length) return;
      var all = choices.map(function (c) { return c.value; });
      var set = p[field] === "*" ? all : p[field];
      var head = el(
        "div",
        { class: "emp-head" },
        el("span", null, label, el("em", null, set == null ? " Any" : p[field] === "*" ? " All" : " " + set.length + " picked")),
        el("button", { class: "linkish", onclick: function () { p[field] = null; profileChanged(); } }, "Any"),
        extra ? extra.map(function (x) { return el("button", { class: "linkish", onclick: function () { p[field] = x.value(); profileChanged(); } }, x.label); }) : null
      );
      var list = el(
        "div",
        { class: "chips" },
        choices.map(function (c) {
          var on = !!set && set.indexOf(c.value) >= 0;
          return el(
            "button",
            {
              class: "chip toggle" + (on ? " on" : ""),
              "aria-pressed": on ? "true" : "false",
              title: c.value,
              onclick: function () {
                var cur = set ? set.slice() : [];
                var i = cur.indexOf(c.value);
                if (i >= 0) cur.splice(i, 1);
                else cur.push(c.value);
                p[field] = cur.length === all.length ? "*" : cur;
                profileChanged();
              },
            },
            c.name
          );
        })
      );
      box.appendChild(el("div", { class: "emp-field wide" }, head, list));
    }

    select("origin", "Origin", o.origin, { value: "__other", name: "Another origin" });
    select("authority", "Authority", o.authority, { value: "__other", name: "Another authority" });
    select("archetype", "Founder species", o.archetype, { value: "__other", name: "Another species type" });
    select("ships", "Ships", o.ships, { value: "__none", name: "Regular ships" });
    if (o.nomadic) select("nomadic", "Nomadic", [{ value: "__no", name: "No" }, { value: "__yes", name: "Yes" }]);
    chips("ethics", "Ethics", o.ethics);
    chips("civics", "Civics", o.civics);
    chips("dlcs", "DLC", o.dlcs, [
      { label: "All", value: function () { return "*"; } },
      { label: "None", value: function () { return []; } },
    ]);
  }

  function wireEmpire() {
    var btn = $("#empire-btn"), panel = $("#empire");
    btn.addEventListener("click", function () {
      panel.hidden = !panel.hidden;
      btn.setAttribute("aria-expanded", panel.hidden ? "false" : "true");
      if (!panel.hidden) {
        $("#filters").hidden = true;
        $("#filters-btn").setAttribute("aria-expanded", "false");
      }
    });
    $("#filters-btn").addEventListener("click", function () {
      panel.hidden = true;
      btn.setAttribute("aria-expanded", "false");
    });
    $("#show-unavail").addEventListener("change", function (e) {
      state.showAll = e.target.checked;
      profileChanged();
    });
    $("#empire-reset").addEventListener("click", function () {
      state.profile = ST.emptyProfile();
      profileChanged();
    });
  }

  wire();
  wireEmpire();
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
    if (h.emp) state.profile = ST.decodeProfile(h.emp);
    return loadDataset(h.ds || (list[0] && list[0].id));
  });
})();

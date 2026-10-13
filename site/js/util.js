// Shared helpers. Everything hangs off window.ST so the site works as plain scripts.
(function () {
  "use strict";
  var ST = (window.ST = {});

  ST.$ = function (sel, root) {
    return (root || document).querySelector(sel);
  };

  // el("div", {class: "x", onclick: fn}, child, "text", [more children])
  ST.el = function (tag, attrs) {
    var node = document.createElement(tag);
    if (attrs) {
      Object.keys(attrs).forEach(function (k) {
        var v = attrs[k];
        if (v == null || v === false) return;
        if (k.slice(0, 2) === "on") node.addEventListener(k.slice(2), v);
        else if (k === "style" && typeof v === "object") {
          Object.keys(v).forEach(function (p) {
            if (p.slice(0, 2) === "--") node.style.setProperty(p, v[p]);
            else node.style[p] = v[p];
          });
        }
        else node.setAttribute(k, v === true ? "" : v);
      });
    }
    for (var i = 2; i < arguments.length; i++) append(node, arguments[i]);
    return node;
  };

  function append(node, c) {
    if (c == null || c === false) return;
    if (Array.isArray(c)) c.forEach(function (x) { append(node, x); });
    else node.appendChild(c.nodeType ? c : document.createTextNode(String(c)));
  }

  ST.clear = function (node) {
    while (node.firstChild) node.removeChild(node.firstChild);
    return node;
  };

  ST.fmtNum = function (n) {
    if (n == null || isNaN(n)) return "–";
    if (Math.abs(n) >= 1000) return Math.round(n).toLocaleString("en-US");
    return String(Math.round(n * 100) / 100);
  };

  ST.humanize = function (key) {
    var s = String(key).replace(/_/g, " ").trim();
    return s.charAt(0).toUpperCase() + s.slice(1);
  };

  // Friendly labels for unlock kinds. Unknown kinds (new game versions, mods) are humanized.
  var KIND = {
    modifier: "Modifier",
    tooltip: "Effect",
    feature: "Feature",
    custom: "Unlock",
    ship: "Ship type",
    component: "Component",
    component_templates: "Component",
    section_templates: "Ship section",
    ship_sizes: "Ship size",
    starbase_modules: "Starbase module",
    starbase_buildings: "Starbase building",
    buildings: "Building",
    megastructures: "Megastructure",
    edicts: "Edict",
    zones: "Zone",
    districts: "District",
    armies: "Army",
    traits: "Trait",
    deposits: "Deposit",
    strategic_resources: "Resource",
    decisions: "Decision",
    policies: "Policy",
    diplo_action: "Diplomacy",
    bypass: "Hyperlane bypass",
  };
  ST.kindLabel = function (kind) {
    if (KIND[kind]) return KIND[kind];
    var h = ST.humanize(kind);
    return /[^s]s$/.test(h) ? h.slice(0, -1) : h;
  };

  // Area accent colours; unknown (modded) areas get a stable hue from their key.
  var AREA = { physics: "#4ea8ff", society: "#5fd38d", engineering: "#f2a33a" };
  ST.areaColor = function (key) {
    if (AREA[key]) return AREA[key];
    var h = 0;
    for (var i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) % 360;
    return "hsl(" + h + " 70% 62%)";
  };

  ST.debounce = function (fn, ms) {
    var t;
    return function () {
      var args = arguments, self = this;
      clearTimeout(t);
      t = setTimeout(function () { fn.apply(self, args); }, ms);
    };
  };

  ST.store = {
    get: function (k, d) {
      try {
        var v = localStorage.getItem("st:" + k);
        return v == null ? d : JSON.parse(v);
      } catch (e) {
        return d;
      }
    },
    set: function (k, v) {
      try { localStorage.setItem("st:" + k, JSON.stringify(v)); } catch (e) { /* private mode */ }
    },
  };
})();

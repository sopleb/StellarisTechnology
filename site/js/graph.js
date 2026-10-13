// Prerequisite graph: layered left-to-right layout, SVG edges, HTML nodes, pan and zoom.
(function () {
  "use strict";
  var ST = window.ST, el = ST.el;
  var NODE_W = 212, NODE_H = 52, COL_GAP = 96, ROW_GAP = 12, PAD = 48;
  var ROW = NODE_H + ROW_GAP;
  var GAP = 56, BPAD = 18, LABEL_H = 22;
  var SVGNS = "http://www.w3.org/2000/svg";

  function avg(xs) {
    var s = 0;
    for (var i = 0; i < xs.length; i++) s += xs[i];
    return s / xs.length;
  }

  // Layout one area (or "all"). The prerequisite graph splits into many independent chains,
  // so each connected group is laid out on its own (layer = longest prerequisite path, rows
  // from barycenter sweeps), then the groups are shelf-packed into a roughly screen-shaped
  // area. Techs with no links in view are gathered into one compact block, ordered by tier.
  function layout(model, area) {
    var list = model.techs.filter(function (t) { return area === "all" || t.area === area; });
    var inSet = new Set(list.map(function (t) { return t.key; }));
    var parents = new Map(), kids = new Map();
    list.forEach(function (t) {
      parents.set(t.key, (t.prerequisites || []).filter(function (p) { return inSet.has(p) && p !== t.key; }));
      kids.set(t.key, []);
    });
    list.forEach(function (t) { parents.get(t.key).forEach(function (p) { kids.get(p).push(t.key); }); });

    // Connected groups (union-find over in-view edges).
    var up = new Map();
    function find(k) {
      while (up.get(k) !== k) {
        up.set(k, up.get(up.get(k)));
        k = up.get(k);
      }
      return k;
    }
    list.forEach(function (t) { up.set(t.key, t.key); });
    list.forEach(function (t) { parents.get(t.key).forEach(function (p) { up.set(find(p), find(t.key)); }); });
    var groups = new Map();
    var loose = [];
    list.forEach(function (t) {
      if (!parents.get(t.key).length && !kids.get(t.key).length) return loose.push(t);
      var r = find(t.key);
      if (!groups.has(r)) groups.set(r, []);
      groups.get(r).push(t);
    });

    var blocks = Array.from(groups.values()).map(function (g) { return layoutGroup(model, g, parents, kids); });
    if (loose.length) blocks.push(layoutLoose(model, loose));

    // Shelf packing, tallest first, towards a 16:10 overall shape.
    blocks.sort(function (a, b) { return b.h - a.h || b.w - a.w; });
    var area2 = 0, widest = 0;
    blocks.forEach(function (b) { area2 += (b.w + GAP) * (b.h + GAP); widest = Math.max(widest, b.w); });
    var target = Math.max(widest, Math.sqrt(area2 * 1.6));
    var x = PAD, y = PAD, shelf = 0, maxX = 0;
    blocks.forEach(function (b) {
      if (x > PAD && x + b.w > PAD + target) {
        x = PAD;
        y += shelf + GAP;
        shelf = 0;
      }
      b.x = x;
      b.y = y;
      x += b.w + GAP;
      shelf = Math.max(shelf, b.h);
      maxX = Math.max(maxX, b.x + b.w);
    });

    var nodes = [];
    blocks.forEach(function (b) {
      b.nodes.forEach(function (n) { nodes.push({ key: n.key, x: b.x + n.x, y: b.y + n.y }); });
    });
    var edges = [];
    list.forEach(function (t) { parents.get(t.key).forEach(function (p) { edges.push({ from: p, to: t.key }); }); });
    return {
      nodes: nodes,
      edges: edges,
      blocks: blocks.map(function (b) { return { x: b.x, y: b.y, w: b.w, h: b.h, label: b.label }; }),
      width: maxX + PAD,
      height: y + shelf + PAD,
    };
  }

  function layoutLoose(model, list) {
    list.sort(function (a, b) {
      return (parseInt(a.tier, 10) || 0) - (parseInt(b.tier, 10) || 0) || a.name.localeCompare(b.name);
    });
    var cols = Math.max(1, Math.ceil(Math.sqrt(list.length / 3)));
    var rows = Math.ceil(list.length / cols);
    return {
      label: "No prerequisite links in view",
      nodes: list.map(function (t, i) {
        return { key: t.key, x: BPAD + Math.floor(i / rows) * (NODE_W + COL_GAP / 2), y: BPAD + LABEL_H + (i % rows) * ROW };
      }),
      w: BPAD * 2 + cols * NODE_W + (cols - 1) * (COL_GAP / 2),
      h: BPAD * 2 + LABEL_H + rows * ROW - ROW_GAP,
    };
  }

  function layoutGroup(model, list, parents, kids) {
    var layer = new Map(), visiting = new Set();
    function depth(k) {
      if (layer.has(k)) return layer.get(k);
      if (visiting.has(k)) return 0;
      visiting.add(k);
      var d = 0;
      parents.get(k).forEach(function (p) { d = Math.max(d, depth(p) + 1); });
      visiting.delete(k);
      layer.set(k, d);
      return d;
    }
    list.forEach(function (t) { depth(t.key); });
    // Pull sources right, next to their earliest dependent.
    list.forEach(function (t) {
      if (parents.get(t.key).length) return;
      var ks = kids.get(t.key);
      layer.set(t.key, Math.max(0, Math.min.apply(null, ks.map(function (k) { return layer.get(k); })) - 1));
    });
    var maxL = 0;
    layer.forEach(function (v) { maxL = Math.max(maxL, v); });

    var layers = [];
    for (var i = 0; i <= maxL; i++) layers.push([]);
    var catIdx = function (t) { return model.catOrder.indexOf((t.category || [])[0]); };
    list
      .slice()
      .sort(function (a, b) { return catIdx(a) - catIdx(b) || a.name.localeCompare(b.name); })
      .forEach(function (t) { layers[layer.get(t.key)].push(t.key); });

    var y = new Map();
    layers.forEach(function (L) { L.forEach(function (k, i) { y.set(k, i); }); });
    for (var it = 0; it < 12; it++) {
      var down = it % 2 === 0;
      for (var s = 0; s <= maxL; s++) {
        var l = down ? s : maxL - s;
        var nb = down ? parents : kids;
        var key = new Map();
        layers[l].forEach(function (k) {
          var ns = nb.get(k);
          key.set(k, ns.length ? avg(ns.map(function (n) { return y.get(n); })) : y.get(k));
        });
        layers[l].sort(function (a, b) { return key.get(a) - key.get(b); });
        layers[l].forEach(function (k, i) { y.set(k, i); });
      }
    }

    // Rows relaxed towards neighbours, never overlapping, never taller than the
    // tallest column plus a little slack.
    var tallest = Math.max.apply(null, layers.map(function (L) { return L.length; }));
    var limit = tallest + Math.ceil(tallest * 0.25);
    for (var pass = 0; pass < 6; pass++) {
      var useParents = pass % 2 === 0;
      for (var s2 = 0; s2 <= maxL; s2++) {
        var l2 = useParents ? s2 : maxL - s2;
        var L = layers[l2];
        if (!L.length) continue;
        var want = L.map(function (k) {
          var ns = (useParents ? parents : kids).get(k);
          return ns.length ? avg(ns.map(function (n) { return y.get(n); })) : y.get(k);
        });
        var pos = [];
        for (var j = 0; j < L.length; j++) pos.push(j === 0 ? Math.max(0, want[0]) : Math.max(want[j], pos[j - 1] + 1));
        pos[L.length - 1] = Math.min(pos[L.length - 1], limit - 1);
        for (var b = L.length - 2; b >= 0; b--) pos[b] = Math.min(pos[b], pos[b + 1] - 1);
        var top = Math.min.apply(null, pos);
        if (top < 0) pos = pos.map(function (p) { return p - top; });
        L.forEach(function (k, n) { y.set(k, pos[n]); });
      }
    }
    var maxY = 0;
    list.forEach(function (t) { maxY = Math.max(maxY, y.get(t.key)); });
    return {
      nodes: list.map(function (t) {
        return { key: t.key, x: BPAD + layer.get(t.key) * (NODE_W + COL_GAP), y: BPAD + y.get(t.key) * ROW };
      }),
      w: BPAD * 2 + (maxL + 1) * NODE_W + maxL * COL_GAP,
      h: BPAD * 2 + maxY * ROW + NODE_H,
    };
  }

  ST.graphLayout = function (model, area) {
    if (!model.layouts.has(area)) model.layouts.set(area, layout(model, area));
    return model.layouts.get(area);
  };

  ST.renderGraph = function (root, model, area, onSelect) {
    ST.clear(root);
    var lay = ST.graphLayout(model, area);
    var pos = new Map(lay.nodes.map(function (n) { return [n.key, n]; }));

    var svg = document.createElementNS(SVGNS, "svg");
    svg.setAttribute("class", "edges");
    svg.setAttribute("width", lay.width);
    svg.setAttribute("height", lay.height);
    lay.blocks.forEach(function (b) {
      var r = document.createElementNS(SVGNS, "rect");
      r.setAttribute("class", "block");
      r.setAttribute("x", b.x);
      r.setAttribute("y", b.y);
      r.setAttribute("width", b.w);
      r.setAttribute("height", b.h);
      r.setAttribute("rx", 14);
      svg.appendChild(r);
      if (b.label) {
        var tx = document.createElementNS(SVGNS, "text");
        tx.setAttribute("x", b.x + BPAD);
        tx.setAttribute("y", b.y + BPAD + 12);
        tx.textContent = b.label;
        svg.appendChild(tx);
      }
    });
    var edgeEls = lay.edges.map(function (e) {
      var a = pos.get(e.from), b = pos.get(e.to);
      var x1 = a.x + NODE_W, y1 = a.y + NODE_H / 2, x2 = b.x, y2 = b.y + NODE_H / 2;
      var dx = Math.max(40, (x2 - x1) * 0.45);
      var p = document.createElementNS(SVGNS, "path");
      p.setAttribute("d", "M" + x1 + " " + y1 + "C" + (x1 + dx) + " " + y1 + " " + (x2 - dx) + " " + y2 + " " + x2 + " " + y2);
      svg.appendChild(p);
      return { from: e.from, to: e.to, el: p };
    });

    var nodeEls = new Map();
    var nodesLayer = el("div", { class: "nodes" });
    lay.nodes.forEach(function (n) {
      var t = model.byKey.get(n.key);
      var external = (t.prerequisites || []).filter(function (p) { return !pos.has(p); }).length;
      var flags = ST.flags(t);
      var node = el(
        "div",
        {
          class: "node" + (t.is_rare ? " rare" : "") + (t.is_dangerous ? " dangerous" : "") + (t.is_event ? " event" : "") + (t.is_repeatable ? " repeatable" : ""),
          "data-key": t.key,
          title: t.name + "\n" + t.key,
          style: { left: n.x + "px", top: n.y + "px", width: NODE_W + "px", height: NODE_H + "px", "--area": ST.areaColor(t.area) },
        },
        el("span", { class: "n-name" }, t.name),
        el(
          "span",
          { class: "n-sub" },
          el("span", { class: "n-tier" }, t.tier != null ? "T" + t.tier : ""),
          el("span", { class: "n-cost" }, ST.fmtNum(t.cost)),
          flags.length ? el("span", { class: "n-flags" }, flags.map(function (f) { return el("i", { class: "dot " + f[0], title: f[1] }); })) : null,
          external ? el("span", { class: "n-ext", title: external + " prerequisite(s) in another area" }, "↰" + external) : null
        )
      );
      nodeEls.set(t.key, node);
      nodesLayer.appendChild(node);
    });

    var world = el("div", { class: "world", style: { width: lay.width + "px", height: lay.height + "px" } }, svg, nodesLayer);
    var hud = el(
      "div",
      { class: "hud" },
      el("button", { class: "icon", title: "Zoom in", "aria-label": "Zoom in", onclick: function () { zoomBy(1.25); } }, "+"),
      el("button", { class: "icon", title: "Zoom out", "aria-label": "Zoom out", onclick: function () { zoomBy(0.8); } }, "−"),
      el("button", { class: "icon", title: "Fit to screen", "aria-label": "Fit to screen", onclick: function () { fit(); } }, "⤢")
    );
    var vp = el("div", { class: "viewport" }, world);
    root.appendChild(el("div", { class: "graph-view" }, vp, hud));

    // ---- pan / zoom -------------------------------------------------------
    var view = { x: 0, y: 0, k: 1 };
    function apply() {
      world.style.transform = "translate(" + view.x + "px," + view.y + "px) scale(" + view.k + ")";
      vp.classList.toggle("far", view.k < 0.45);
    }
    function clampK(k) { return Math.max(0.08, Math.min(2.2, k)); }
    function zoomAt(k, cx, cy) {
      k = clampK(k);
      view.x = cx - (cx - view.x) * (k / view.k);
      view.y = cy - (cy - view.y) * (k / view.k);
      view.k = k;
      apply();
    }
    function zoomBy(f) { zoomAt(view.k * f, vp.clientWidth / 2, vp.clientHeight / 2); }
    function fit() {
      var k = clampK(Math.min(vp.clientWidth / lay.width, vp.clientHeight / lay.height, 1));
      view.k = k;
      view.x = (vp.clientWidth - lay.width * k) / 2;
      view.y = Math.max(0, (vp.clientHeight - lay.height * k) / 2);
      apply();
    }
    function center(key, minK) {
      var n = pos.get(key);
      if (!n) return;
      var k = Math.max(view.k, minK || 0.75);
      view.k = clampK(k);
      view.x = vp.clientWidth / 2 - (n.x + NODE_W / 2) * view.k;
      view.y = vp.clientHeight / 2 - (n.y + NODE_H / 2) * view.k;
      world.classList.add("animate");
      apply();
      setTimeout(function () { world.classList.remove("animate"); }, 320);
    }

    vp.addEventListener("wheel", function (e) {
      e.preventDefault();
      var r = vp.getBoundingClientRect();
      var f = Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0016));
      zoomAt(view.k * f, e.clientX - r.left, e.clientY - r.top);
    }, { passive: false });

    var pointers = new Map(), moved = 0, last = null, pinch = null;
    vp.addEventListener("pointerdown", function (e) {
      if (e.button !== 0 && e.pointerType === "mouse") return;
      vp.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      moved = 0;
      last = { x: e.clientX, y: e.clientY };
      if (pointers.size === 2) {
        var ps = Array.from(pointers.values());
        pinch = { d: Math.hypot(ps[0].x - ps[1].x, ps[0].y - ps[1].y), k: view.k };
      }
    });
    vp.addEventListener("pointermove", function (e) {
      if (!pointers.has(e.pointerId)) return;
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pointers.size === 2 && pinch) {
        var ps = Array.from(pointers.values());
        var r = vp.getBoundingClientRect();
        var d = Math.hypot(ps[0].x - ps[1].x, ps[0].y - ps[1].y);
        zoomAt(pinch.k * (d / pinch.d), (ps[0].x + ps[1].x) / 2 - r.left, (ps[0].y + ps[1].y) / 2 - r.top);
        moved = 99;
        return;
      }
      var dx = e.clientX - last.x, dy = e.clientY - last.y;
      moved += Math.abs(dx) + Math.abs(dy);
      last = { x: e.clientX, y: e.clientY };
      if (moved > 4) {
        vp.classList.add("dragging");
        view.x += dx;
        view.y += dy;
        apply();
      }
    });
    function up(e) {
      if (!pointers.has(e.pointerId)) return;
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinch = null;
      vp.classList.remove("dragging");
      if (e.type === "pointerup" && moved <= 4 && pointers.size === 0) {
        var target = document.elementFromPoint(e.clientX, e.clientY);
        var node = target && target.closest && target.closest(".node");
        if (node) onSelect(node.getAttribute("data-key"), false);
      }
    }
    vp.addEventListener("pointerup", up);
    vp.addEventListener("pointercancel", up);

    // Keyboard panning when the viewport has focus.
    vp.tabIndex = 0;
    vp.addEventListener("keydown", function (e) {
      var step = 80;
      var map = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
      if (map[e.key]) {
        view.x += map[e.key][0];
        view.y += map[e.key][1];
        apply();
        e.preventDefault();
      } else if (e.key === "+" || e.key === "=") zoomBy(1.25);
      else if (e.key === "-") zoomBy(0.8);
      else if (e.key === "0") fit();
    });

    // ---- state ------------------------------------------------------------
    var current = { match: null, sel: null };
    function paint() {
      var sel = current.sel && pos.has(current.sel) ? current.sel : null;
      var anc = sel ? ST.ancestors(model, sel) : null;
      var desc = sel ? ST.descendants(model, sel) : null;
      world.classList.toggle("focus", !!sel);
      nodeEls.forEach(function (node, key) {
        var t = model.byKey.get(key);
        var ok = !current.match || current.match(t);
        node.classList.toggle("dim", !ok);
        node.classList.toggle("sel", key === sel);
        node.classList.toggle("anc", !!anc && anc.has(key));
        node.classList.toggle("desc", !!desc && desc.has(key));
      });
      edgeEls.forEach(function (e) {
        var onAnc = anc && (anc.has(e.to) || e.to === sel) && anc.has(e.from);
        var onDesc = desc && (desc.has(e.from) || e.from === sel) && desc.has(e.to);
        e.el.setAttribute("class", onAnc ? "anc" : onDesc ? "desc" : "");
      });
    }

    requestAnimationFrame(function () {
      if (current.sel && pos.has(current.sel)) center(current.sel, 0.9);
      else fit();
    });
    return {
      shown: lay.nodes.length,
      setMatch: function (m) { current.match = m; paint(); },
      select: function (key, scroll) {
        current.sel = key;
        paint();
        if (key && scroll && vp.clientWidth) center(key);
      },
      center: center,
      fit: fit,
      has: function (key) { return pos.has(key); },
    };
  };
})();

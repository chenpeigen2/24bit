// 24bit — frontend.
//
// Runs inside the Tauri webview when packaged; falls back to demo data in a
// plain browser so the layout can be rendered headlessly for review.

const isTauri = typeof window.__TAURI__ !== "undefined";
const invoke = isTauri ? window.__TAURI__.core.invoke : demoInvoke;
const listen = isTauri ? window.__TAURI__.event.listen : async () => () => {};

const $ = (id) => document.getElementById(id);
const el = (tag, cls, text) => {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text != null) node.textContent = text;
  return node;
};

const state = {
  page: "search",
  results: [],
  selected: 0,
  playingId: null,
  pending: null,
  lyrics: [],
  current: null,
  quality: "master",
};

// ---------------------------------------------------------------- rendering

function icon(paths, { fill = false } = {}) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  if (fill) svg.setAttribute("fill", "currentColor");
  for (const d of paths) {
    const p = document.createElementNS("http://www.w3.org/2000/svg", "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

const ICON_PLAY = ["M9 6.5L18 12 9 17.5z"];
const ICON_PAUSE = ["M8.5 6.5h3v11h-3z", "M13 6.5h3v11h-3z"];
const ICON_DOWN = ["M12 5v10", "M7.5 11.5L12 16l4.5-4.5", "M5.5 19h13"];

function renderResults() {
  const list = $("results");
  list.replaceChildren();
  const empty = $("empty");
  const count = $("result-count");

  if (!state.results.length) {
    empty.hidden = state.q === "" ? false : true;
    if (state.q && state.q !== "") {
      empty.hidden = false;
      empty.querySelector("h2").textContent = state.searching ? "正在搜索…" : "没有找到匹配的歌曲";
    }
    count.textContent = "";
    return;
  }
  empty.hidden = true;
  count.textContent = `${state.results.length} 个结果`;

  state.results.forEach((item, i) => {
    const row = el("div", "row");
    if (i === state.selected) row.classList.add("is-selected");
    if (item.id === state.playingId) row.classList.add("is-playing");
    if (item.id === state.pending) row.classList.add("is-pending");

    const art = el("div", "row-art");
    if (item.cover) art.style.backgroundImage = `url("${item.cover}")`;

    const text = el("div", "row-text");
    text.append(el("div", "row-title", item.name));
    text.append(el("div", "row-sub", `${item.player} · ${item.album}`));

    const actions = el("div", "row-actions");

    if (item.id === state.pending) {
      actions.append(ring(state.progress ?? 0));
    } else {
      const play = el("button", "icon-btn");
      play.append(icon(item.id === state.playingId ? ICON_PAUSE : ICON_PLAY, { fill: true }));
      if (item.id !== state.playingId) play.classList.add("solid");
      play.onclick = (e) => { e.stopPropagation(); toggle(item); };
      actions.append(play);
    }

    const dl = el("button", "icon-btn");
    dl.append(icon(ICON_DOWN));
    dl.onclick = (e) => { e.stopPropagation(); download(item); };

    actions.append(dl);
    row.append(art, text, actions);
    row.onclick = () => { state.selected = i; renderResults(); renderInspector(); };
    list.append(row);
  });
}

function ring(fraction) {
  const wrap = el("div", "ring");
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 26 26");
  const r = 10, c = 2 * Math.PI * r;
  for (const cls of ["track", "value"]) {
    const circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
    circle.setAttribute("cx", "13"); circle.setAttribute("cy", "13"); circle.setAttribute("r", r);
    circle.setAttribute("class", cls);
    if (cls === "value") {
      circle.setAttribute("stroke-dasharray", c);
      circle.setAttribute("stroke-dashoffset", c * (1 - fraction));
    }
    svg.appendChild(circle);
  }
  wrap.append(svg, el("span", null, `${Math.round(fraction * 100)}`));
  return wrap;
}

function renderInspector() {
  const item = state.results[state.selected];
  const pane = $("inspector");
  if (!item) { pane.hidden = true; return; }
  pane.hidden = false;

  $("insp-art").style.backgroundImage = item.cover ? `url("${item.cover}")` : "";
  $("insp-title").textContent = item.name;
  $("insp-artist").textContent = item.player;
  $("insp-album").textContent = item.album;

  const meta = $("insp-meta");
  meta.replaceChildren();
  for (const text of [qualityName(state.quality), "FLAC", "24bit / 192kHz"]) {
    meta.append(el("span", null, text));
  }

  const lyrics = $("lyrics");
  lyrics.replaceChildren();
  if (item.id === state.playingId && state.lyrics.length) {
    const pos = state.position ?? 0;
    const idx = state.lyrics.findIndex((l, i) =>
      l.at <= pos && (i + 1 === state.lyrics.length || state.lyrics[i + 1].at > pos));
    state.lyrics.forEach((line, i) => {
      const row = el("div", i === idx ? "is-current" : null, line.text);
      row.onclick = () => invoke("seek", { seconds: line.at / 1000 });
      lyrics.append(row);
    });
  }
}

function renderPlayer(snapshot) {
  state.position = snapshot.position ?? 0;
  const playing = snapshot.playing;
  $("t-pos").textContent = fmt(snapshot.position);
  $("t-dur").textContent = fmt(snapshot.duration);
  const pct = snapshot.duration ? (snapshot.position / snapshot.duration) * 100 : 0;
  $("bar-fill").style.width = `${pct}%`;
  $("bar-knob").style.left = `${pct}%`;

  const play = $("play");
  play.replaceChildren(icon(playing ? ICON_PAUSE : ICON_PLAY, { fill: true }));
  $("p-quality").textContent = snapshot.quality ?? "";

  if (snapshot.title) {
    $("p-title").textContent = snapshot.title;
    $("p-artist").textContent = snapshot.artist ?? "";
    if (snapshot.cover) $("p-art").style.backgroundImage = `url("${snapshot.cover}")`;
    $("now-card").hidden = false;
    $("now-title").textContent = snapshot.title;
    $("now-artist").textContent = snapshot.artist ?? "";
    if (snapshot.cover) $("now-art").style.backgroundImage = `url("${snapshot.cover}")`;
  }
}

const qualityName = (q) => (q === "surround" ? "高清环绕声" : "至臻母带");

const fmt = (ms) => {
  if (!ms) return "0:00";
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
};

// ------------------------------------------------------------------ actions

async function search(q) {
  state.q = q;
  state.searching = true;
  renderResults();
  const results = await invoke("search", { keyword: q });
  state.searching = false;
  state.results = results;
  state.selected = 0;
  renderResults();
  renderInspector();
}

async function toggle(item) {
  if (item.id === state.playingId) {
    await invoke("pause");
    state.playingId = null;
  } else {
    state.pending = item.id;
    state.progress = 0;
    renderResults();
    const detail = await invoke("prepare", { id: item.id, quality: state.quality });
    state.pending = null;
    state.playingId = detail.id;
    state.lyrics = detail.lyrics ?? [];
    renderResults();
    renderInspector();
  }
}

async function download(item) {
  state.pending = item.id;
  renderResults();
  await invoke("export", { id: item.id, quality: state.quality });
  state.pending = null;
  renderResults();
}

// ------------------------------------------------------------------- wiring

let timer = null;
$("q").addEventListener("input", (e) => {
  clearTimeout(timer);
  const value = e.target.value.trim();
  if (!value) return;
  timer = setTimeout(() => search(value), 320);
});
$("q").addEventListener("keydown", (e) => {
  if (e.key === "Enter") { clearTimeout(timer); const v = e.target.value.trim(); if (v) search(v); }
});

document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.onclick = () => {
    document.querySelectorAll(".nav-item").forEach((b) => b.classList.toggle("is-active", b === btn));
    state.page = btn.dataset.page;
    document.querySelectorAll(".page").forEach((p) => { p.hidden = p.dataset.page !== state.page; });
    if (state.page === "charts") loadCharts();
    if (state.page === "library") loadLibrary();
    if (state.page === "settings") loadSettings();
  };
});

$("play").onclick = () => invoke("toggle");
$("prev").onclick = () => invoke("restart");
$("mute").onclick = () => invoke("mute");
$("volume").oninput = (e) => invoke("set_volume", { volume: Number(e.target.value) });
$("insp-play").onclick = () => { const it = state.results[state.selected]; if (it) toggle(it); };
$("insp-dl").onclick = () => { const it = state.results[state.selected]; if (it) download(it); };
$("empty-cta").onclick = () => document.querySelector('[data-page="charts"]').click();

const bar = $("bar");
const seekAt = (e) => {
  const rect = bar.getBoundingClientRect();
  const f = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  invoke("seek", { fraction: f });
};
bar.onclick = seekAt;
bar.addEventListener("pointerdown", () => bar.classList.add("is-dragging"));
addEventListener("pointerup", () => bar.classList.remove("is-dragging"));

listen("player", (event) => renderPlayer(event.payload));
listen("progress", (event) => {
  if (event.payload.id === state.pending) {
    state.progress = event.payload.total ? event.payload.done / event.payload.total : 0;
    renderResults();
    setStatus(`下载中 ${Math.round(state.progress * 100)}%`);
  }
});

function setStatus(text) {
  const s = $("status");
  s.replaceChildren();
  if (text) {
    s.append(el("span", "dot"));
    s.append(el("span", null, text));
  }
  s.append(el("span", "grow"));
  s.append(el("span", "ver", "v0.1.0"));
}

async function loadCharts() {
  const charts = await invoke("charts");
  const wrap = $("charts");
  wrap.replaceChildren();
  for (const [title, rows] of [["歌曲", charts.songs], ["歌手", charts.artists]]) {
    const col = el("div", "chart");
    col.append(el("h3", null, title));
    rows.forEach((r, i) => {
      const row = el("div", "chart-row");
      row.append(el("div", "chart-rank", String(i + 1)));
      row.append(el("div", "chart-name", r.name));
      row.append(el("div", "chart-plays", String(r.plays)));
      row.onclick = () => { document.querySelector('[data-page="search"]').click(); $("q").value = r.name; search(r.name); };
      col.append(row);
    });
    wrap.append(col);
  }
}

async function loadLibrary() {
  const rows = await invoke("library");
  const list = $("library");
  list.replaceChildren();
  if (!rows.length) {
    list.append(el("div", "list-meta", "还没有缓存的曲目"));
    return;
  }
  rows.forEach((r) => {
    const row = el("div", "row");
    const text = el("div", "row-text");
    text.append(el("div", "row-title", r.title));
    text.append(el("div", "row-sub", r.subtitle || ""));
    const actions = el("div", "row-actions");
    const play = el("button", "icon-btn");
    play.append(icon(ICON_PLAY, { fill: true }));
    play.onclick = () => invoke("play_file", { path: r.path });
    actions.append(play);
    row.append(text, actions);
    list.append(row);
  });
}

async function loadSettings() {
  const cfg = await invoke("settings");
  const wrap = $("settings");
  wrap.replaceChildren();

  const quality = el("div", "section");
  quality.append(el("h3", null, "音质偏好"));
  const card = el("div", "card");
  for (const [value, title, detail] of [
    ["master", "至臻母带", "24bit / 192kHz FLAC，含歌词"],
    ["surround", "高清环绕声", "较小体积，不含歌词"],
  ]) {
    const row = el("div", "card-row" + (cfg.quality === value ? " is-selected" : ""));
    row.append(el("div", "radio"));
    const text = el("div", "grow");
    text.append(el("div", "title", title), el("div", "detail", detail));
    row.append(text);
    row.onclick = async () => { state.quality = value; await invoke("set_quality", { quality: value }); loadSettings(); };
    card.append(row);
  }
  quality.append(card);
  wrap.append(quality);

  const storage = el("div", "section");
  storage.append(el("h3", null, "存储"));
  const c2 = el("div", "card");
  const r1 = el("div", "card-row");
  r1.append(el("div", "grow", null));
  r1.querySelector(".grow").append(el("div", "title", `${cfg.files} 个文件 · ${cfg.bytes}`));
  r1.querySelector(".grow").append(el("div", "mono", cfg.cache_dir));
  c2.append(r1);
  const r2 = el("div", "card-row");
  const clear = el("button", "btn-secondary", "清空缓存");
  clear.onclick = async () => {
    if (clear.dataset.armed) { await invoke("clear_cache"); loadSettings(); }
    else { clear.dataset.armed = "1"; clear.textContent = "再点一次确认"; setTimeout(() => { delete clear.dataset.armed; clear.textContent = "清空缓存"; }, 3000); }
  };
  r2.append(clear);
  c2.append(r2);
  storage.append(c2);
  wrap.append(storage);
}

// -------------------------------------------------------------- demo mode

async function demoInvoke(cmd, args = {}) {
  const demo = window.__DEMO__;
  await new Promise((r) => setTimeout(r, 120));
  if (cmd === "search") return demo.results;
  if (cmd === "charts") return demo.charts;
  if (cmd === "library") return demo.library;
  if (cmd === "settings") return demo.settings;
  if (cmd === "prepare") return { id: args.id, lyrics: demo.lyrics };
  if (cmd === "pause") return true;
  return true;
}

if (!isTauri) {
  // Rendered in a browser for review: fill the window with representative data.
  const demo = await import("./demo.js");
  window.__DEMO__ = demo;
  state.results = demo.results;
  state.selected = 0;
  state.q = "富士山下";
  state.playingId = demo.results[0].id;
  state.lyrics = demo.lyrics;
  state.position = 24000;
  $("q").value = "富士山下";
  renderResults();
  renderInspector();
  renderPlayer({
    title: demo.results[0].name,
    artist: demo.results[0].player,
    quality: "至臻母带",
    cover: demo.results[0].cover,
    playing: true,
    position: 24000,
    duration: 258000,
  });
  setStatus("");
  $("cache-line").textContent = `${demo.settings.files} 个文件 · ${demo.settings.bytes}`;
}

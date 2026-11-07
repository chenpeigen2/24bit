/**
 * Sample data for headless layout review. Never shipped behaviour: it only
 * loads when the page is opened in a plain browser instead of the webview.
 */

import type {
  LibraryRow,
  LyricLine,
  SearchItem,
  Settings,
  WeeklyCharts,
} from "./types";

const rosesCover = `data:image/svg+xml;utf8,${encodeURIComponent(`
<svg xmlns="http://www.w3.org/2000/svg" width="320" height="320">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#3b1326"/>
      <stop offset=".5" stop-color="#2a0e1c"/>
      <stop offset="1" stop-color="#170912"/>
    </linearGradient>
    <radialGradient id="bloom" cx=".66" cy=".3" r=".62">
      <stop offset="0" stop-color="#e2688c" stop-opacity=".72"/>
      <stop offset="1" stop-color="#e2688c" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="bloom2" cx=".2" cy=".8" r=".5">
      <stop offset="0" stop-color="#a1325c" stop-opacity=".38"/>
      <stop offset="1" stop-color="#a1325c" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="320" height="320" fill="url(#bg)"/>
  <rect width="320" height="320" fill="url(#bloom)"/>
  <rect width="320" height="320" fill="url(#bloom2)"/>
  <g transform="translate(160 148)" fill="none" stroke="rgba(255,220,230,.42)">
    <ellipse rx="52" ry="20" transform="rotate(-18)"/>
    <ellipse rx="52" ry="20" transform="rotate(30)"/>
    <ellipse rx="52" ry="20" transform="rotate(78)"/>
    <ellipse rx="22" ry="22"/>
    <ellipse rx="11" ry="11"/>
  </g>
  <text x="160" y="262" font-size="12" text-anchor="middle"
        fill="rgba(255,220,232,.66)" font-family="Georgia, serif" letter-spacing="4.4">roses</text>
</svg>`)}`;

const cover = `data:image/svg+xml;utf8,${encodeURIComponent(`
<svg xmlns="http://www.w3.org/2000/svg" width="320" height="320">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2=".35" y2="1">
      <stop offset="0" stop-color="#4a3f52"/>
      <stop offset=".48" stop-color="#2c2433"/>
      <stop offset="1" stop-color="#14101a"/>
    </linearGradient>
    <radialGradient id="warm" cx=".74" cy=".22" r=".62">
      <stop offset="0" stop-color="#f0c38c" stop-opacity=".62"/>
      <stop offset="1" stop-color="#f0c38c" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="rose" cx=".18" cy=".82" r=".55">
      <stop offset="0" stop-color="#c07a86" stop-opacity=".28"/>
      <stop offset="1" stop-color="#c07a86" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="320" height="320" fill="url(#g)"/>
  <rect width="320" height="320" fill="url(#warm)"/>
  <rect width="320" height="320" fill="url(#rose)"/>
  <circle cx="160" cy="134" r="52" fill="none" stroke="rgba(255,255,255,.16)"/>
  <circle cx="160" cy="134" r="32" fill="none" stroke="rgba(255,255,255,.28)"/>
  <circle cx="160" cy="134" r="3" fill="rgba(255,255,255,.5)"/>
  <text x="160" y="256" font-size="13" text-anchor="middle"
        fill="rgba(255,255,255,.52)" font-family="Georgia, serif" letter-spacing="3.6">
    WHAT'S GOING ON
  </text>
</svg>`)}`;

const rows: Array<[string, string, string]> = [
  ["富士山下", "陈奕迅", "What's Going On…?"],
  ["富士山下（Live）", "陈奕迅", "DUO 陈奕迅 2010 演唱会"],
  ["爱情转移", "陈奕迅", "认了吧"],
  ["海阔天空", "Beyond", "华纳廿三周年纪念精选系列"],
  ["晴天", "周杰伦", "叶惠美"],
  ["青花瓷", "周杰伦", "我很忙"],
  ["夜曲", "周杰伦", "十一月的萧邦"],
  ["如愿", "王菲", "如愿"],
];

const results: SearchItem[] = rows.map(([name, player, album], i) => ({
  id: `demo-${i}`,
  name,
  player,
  album,
  cover,
}));

const rosesRows: Array<[string, string, string]> = [
  ["roses", "The Chainsmokers", "Bouquet"],
  ["roses (Imanbek Remix)", "SAINt JHN", "roses (Remixes)"],
  ["roses", "Agnes Obel", "Aventine"],
  ["paper roses", "Gene Vincent", "Blue Jean Bop"],
  ["bed of roses", "Bon Jovi", "Keep the Faith"],
  ["every rose has its thorn", "Poison", "Open Up and Say… Ahh!"],
  ["rose garden", "Lynn Anderson", "Rose Garden"],
  ["wild roses", "Of Monsters and Men", "Beneath the Skin"],
];

const rosesResults: SearchItem[] = rosesRows.map(([name, player, album], i) => ({
  id: `roses-${i}`,
  name,
  player,
  album,
  cover: rosesCover,
}));

/** The first roses track is what the demo shows as playing. */
const rosesFirst = rosesResults[0];

const charts: WeeklyCharts = {
  songs: rows.map((r, i) => ({ name: r[0], plays: 1418 - i * 97 })),
  artists: ["周杰伦", "陈奕迅", "王菲", "Beyond", "张学友", "蔡琴", "邓紫棋", "王杰"].map(
    (name, i) => ({ name, plays: 4300 - i * 310 }),
  ),
};

const library: LibraryRow[] = results.slice(0, 3).map((r, i) => ({
  title: r.name,
  subtitle: `${r.player} · 至臻母带`,
  path: `/tmp/${i}.flac`,
  cover,
}));

const settings: Settings = {
  quality: "master",
  files: 3,
  bytes: "446.7 MB",
  cache_dir: "/Users/you/Library/Caches/24bit",
};

const lyrics: LyricLine[] = [
  "作词 : 林夕",
  "作曲 : Christopher Chak",
  "编曲 : 陈珀 / C. Y. Kong",
  "拦路雨偏似雪花 饮泣的你冻吗",
  "这风褛我给你磨到有襟花",
  "连调了职也不怕 怎么始终牵挂",
  "苦心选中今天想车你回家",
  "原谅我不再送花 伤口应要结疤",
  "花瓣铺满心里坟场才害怕",
  "如若你非我不嫁 彼此终必火化",
  "一生一世等一天需要代价",
].map((text, i) => ({ at: i * 8500, text }));

/** Warm amber — the same family the demo cover glows with. */
const accent: [number, number, number] = [206, 156, 96];

const rosesLyrics: LyricLine[] = [
  "Take it slow, but it's not typical",
  "He already knows that my love is fire",
  "His heart was a stone, but then his hands roam",
  "I turned him to gold and it took him higher",
  "Say you'll never let me go",
  "Deep in my bones, I can feel you",
  "Take me back to a time when we weren't on my mind",
  "I can be myself",
  "Take it slow, but it's not typical",
  "Say you'll never let me go",
].map((text, i) => ({ at: i * 7200, text }));

/** Warm rose — same family the roses cover glows with. */
const rosesAccent: [number, number, number] = [214, 96, 124];

export const demo = {
  results,
  rosesResults,
  rosesLyrics,
  charts,
  library,
  settings,
  lyrics,
  accent,
  rosesAccent,
  cover,
  rosesCover,
  rosesFirst,
};

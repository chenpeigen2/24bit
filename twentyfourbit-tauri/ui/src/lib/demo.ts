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

export const demo = { results, charts, library, settings, lyrics, accent, cover };

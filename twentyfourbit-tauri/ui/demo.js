// Sample data for headless layout review. Not shipped behaviour.
const cover = "data:image/svg+xml;utf8," + encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
     <defs>
       <linearGradient id="g" x1="0" y1="0" x2=".3" y2="1">
         <stop offset="0" stop-color="#3f3d56"/><stop offset=".55" stop-color="#2b2a3d"/>
         <stop offset="1" stop-color="#17161f"/>
       </linearGradient>
       <radialGradient id="glow" cx=".72" cy=".24" r=".7">
         <stop offset="0" stop-color="#e8c88a" stop-opacity=".55"/>
         <stop offset="1" stop-color="#e8c88a" stop-opacity="0"/>
       </radialGradient>
     </defs>
     <rect width="200" height="200" fill="url(#g)"/>
     <rect width="200" height="200" fill="url(#glow)"/>
     <circle cx="100" cy="86" r="30" fill="none" stroke="rgba(255,255,255,.22)" stroke-width="1"/>
     <circle cx="100" cy="86" r="18" fill="none" stroke="rgba(255,255,255,.34)" stroke-width="1"/>
     <text x="100" y="163" font-size="10.5" text-anchor="middle"
           fill="rgba(255,255,255,.55)" font-family="-apple-system" letter-spacing="2.4">WHAT'S GOING ON</text>
   </svg>`);

const rows = [
  ["富士山下", "陈奕迅", "What's Going On…?"],
  ["富士山下 (Live)", "陈奕迅", "DUO 陈奕迅2010演唱会"],
  ["爱情转移", "陈奕迅", "认了吧"],
  ["海阔天空", "Beyond", "华纳廿三周年纪念精选系列"],
  ["晴天", "周杰伦", "叶惠美"],
  ["青花瓷", "周杰伦", "我很忙"],
  ["夜曲", "周杰伦", "十一月的萧邦"],
  ["如愿", "王菲", "如愿"],
];

export const results = rows.map(([name, player, album], i) => ({
  id: `demo-${i}`, name, player, album, cover,
}));

export const charts = {
  songs: rows.map((r, i) => ({ name: r[0], plays: 1418 - i * 97 })),
  artists: ["周杰伦", "陈奕迅", "王菲", "Beyond", "张学友", "蔡琴", "邓紫棋", "王杰"]
    .map((n, i) => ({ name: n, plays: 4300 - i * 310 })),
};

export const library = results.slice(0, 3).map((r, i) => ({
  title: r.name, subtitle: `${r.player} · 至臻母带`, path: `/tmp/${i}.flac`, bytes: 156129336,
}));

export const settings = {
  quality: "master", files: 3, bytes: "446.7 MB",
  cache_dir: "/Users/you/Library/Caches/24bit",
};

export const lyrics = [
  "作词 : 林夕",
  "作曲 : Christopher Chak",
  "编曲 : 陈珀 / C. Y. Kong",
  "拦路雨偏似雪花 饮泣的你冻吗",
  "这风褛我给你磨到有襟花",
  "连调了职也不怕 怎么始终牵挂",
  "苦心选中今天想车你回家",
  "原谅我不再送花 伤口应要结疤",
  "花瓣铺满心里坟场才害怕",
].map((text, i) => ({ at: i * 8500, text }));

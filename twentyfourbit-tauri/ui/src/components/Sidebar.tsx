import type { Page, PlayerSnapshot } from "../lib/types";
import { GearIcon, LibraryIcon, MusicIcon, SearchIcon } from "./Icons";

interface Props {
  page: Page;
  onNavigate: (page: Page) => void;
  cacheLine: string;
  account: string | null;
  snapshot: PlayerSnapshot;
}

const ITEMS: Array<{ page: Page; label: string; icon: React.ReactNode }> = [
  { page: "search", label: "搜索", icon: <SearchIcon size={15} /> },
  { page: "charts", label: "榜单", icon: <MusicIcon size={15} /> },
  { page: "library", label: "音乐库", icon: <LibraryIcon size={15} /> },
  { page: "settings", label: "设置", icon: <GearIcon size={15} /> },
];

export const Sidebar = ({ page, onNavigate, cacheLine, account, snapshot }: Props) => (
  <aside className="sidebar" data-tauri-drag-region>
    <div className="brand" data-tauri-drag-region>
      <span className="brand-mark">24bit</span>
      <span className="brand-sub">HI-RES</span>
    </div>

    <div className="nav-label">浏览</div>
    <nav className="nav">
      {ITEMS.map((item) => (
        <button
          key={item.page}
          className={`nav-item${page === item.page ? " is-active" : ""}`}
          onClick={() => onNavigate(item.page)}
        >
          {item.icon}
          <span>{item.label}</span>
        </button>
      ))}
    </nav>

    <div className="sidebar-foot">
      {snapshot.title && (
        <div className="now-card">
          <div
            className="now-art"
            style={snapshot.cover ? { backgroundImage: `url("${snapshot.cover}")` } : undefined}
          />
          <div className="now-text">
            <div className="now-title">{snapshot.title}</div>
            <div className="now-artist">{snapshot.artist}</div>
          </div>
        </div>
      )}
      <div className="cache-line">{cacheLine}</div>
      <div className="cache-line dim">{account ? `已登录 · ${account}` : "游客"}</div>
    </div>
  </aside>
);

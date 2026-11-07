import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { PlaybackMode, Settings } from "../lib/types";
import type { Theme } from "../hooks/useTheme";
import { MoonIcon, SunIcon } from "./Icons";

const PLAYBACK_MODE_LABEL: Record<PlaybackMode, string> = {
  sequential: "顺序播放",
  "repeat-all": "列表循环",
  "repeat-one": "单曲循环",
  shuffle: "随机播放",
};

const QUALITIES: Array<[value: string, title: string, detail: string]> = [
  ["master", "至臻母带", "24bit / 192kHz FLAC，含歌词"],
  ["surround", "高清环绕声", "较小体积，不含歌词"],
];

interface Props {
  quality: string;
  onQuality: (q: string) => void;
  onToast: (text: string, kind?: "ok" | "err") => void;
  account: string | null;
  onAccount: (name: string) => void;
  theme: Theme;
  onTheme: (theme: Theme) => void;
  playbackMode: PlaybackMode;
  onCyclePlaybackMode: () => void;
}

/** Quality, account and storage. A destructive action arms before it fires. */
export const SettingsPage = ({
  quality,
  onQuality,
  onToast,
  account,
  onAccount,
  theme,
  onTheme,
  playbackMode,
  onCyclePlaybackMode,
}: Props) => {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [armed, setArmed] = useState(false);
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setSettings(await api.settings());
    } catch (e) {
      onToast(String(e), "err");
    }
  }, [onToast]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(false), 3000);
    return () => window.clearTimeout(t);
  }, [armed]);

  return (
    <section className="page settings-page">
      <header className="page-head" data-tauri-drag-region>
        <h1>设置</h1>
        <p>音质、账号与存储</p>
      </header>

      <div className="settings">
        <div className="section">
          <h3>外观</h3>
          <div className="card">
            <div className="card-row">
              <div className="grow">
                <div className="title">主题</div>
                <div className="detail">默认跟随系统</div>
              </div>
              <div className="theme-switch">
                <button
                  className={theme === "light" ? "is-active" : ""}
                  aria-pressed={theme === "light"}
                  onClick={() => onTheme("light")}
                >
                  <SunIcon size={12} />
                  浅色
                </button>
                <button
                  className={theme === "dark" ? "is-active" : ""}
                  aria-pressed={theme === "dark"}
                  onClick={() => onTheme("dark")}
                >
                  <MoonIcon size={12} />
                  深色
                </button>
              </div>
            </div>
          </div>
        </div>

        <div className="section">
          <h3>音质偏好</h3>
          <div className="card" role="radiogroup" aria-label="音质偏好">
            {QUALITIES.map(([value, title, detail]) => (
              <button
                type="button"
                role="radio"
                aria-checked={quality === value}
                key={value}
                className={`card-row${quality === value ? " is-selected" : ""}`}
                onClick={async () => {
                  if (quality === value) return;
                  const previous = quality;
                  onQuality(value);
                  try {
                    await api.setQuality(value);
                  } catch (e) {
                    onQuality(previous);
                    onToast(String(e), "err");
                  }
                }}
              >
                <span className="radio" aria-hidden="true" />
                <span className="grow">
                  <span className="title">{title}</span>
                  <span className="detail">{detail}</span>
                </span>
              </button>
            ))}
          </div>
        </div>

        <div className="section">
          <h3>账号</h3>
          <div className="card">
            <div className="card-row">
              <div className="grow">
                <div className="title">{account ? `已登录 · ${account}` : "游客"}</div>
                <div className="detail">
                  {account ? "可无限下载" : "每天可下载数首"}
                </div>
              </div>
            </div>
            {!account && (
              <div className="card-row">
                <input
                  className="text-input"
                  aria-label="账号"
                  autoComplete="username"
                  placeholder="账号"
                  value={user}
                  onChange={(e) => setUser(e.target.value)}
                />
                <input
                  className="text-input"
                  aria-label="密码"
                  autoComplete="current-password"
                  placeholder="密码"
                  type="password"
                  value={pass}
                  onChange={(e) => setPass(e.target.value)}
                />
                <button
                  className="btn-primary"
                  disabled={busy || !user.trim() || !pass}
                  onClick={async () => {
                    setBusy(true);
                    try {
                      const name = await api.login(user.trim(), pass);
                      onAccount(name);
                      onToast(`已登录 ${name}`);
                    } catch (e) {
                      onToast(String(e), "err");
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  登录
                </button>
              </div>
            )}
          </div>
        </div>

        <div className="section">
          <h3>存储</h3>
          <div className="card">
            <div className="card-row">
              <div className="grow">
                <div className="title">
                  {settings ? `${settings.files} 个文件 · ${settings.bytes}` : "—"}
                </div>
                <div className="mono">{settings?.cache_dir ?? ""}</div>
              </div>
            </div>
            <div className="card-row">
              <button
                className={`btn-secondary${armed ? " danger" : ""}`}
                onClick={async () => {
                  if (!armed) {
                    setArmed(true);
                    return;
                  }
                  try {
                    await api.clearCache();
                    onToast("缓存已清空");
                    void load();
                  } catch (e) {
                    onToast(String(e), "err");
                  } finally {
                    setArmed(false);
                  }
                }}
              >
                {armed ? "再点一次确认" : "清空缓存"}
              </button>
            </div>
          </div>
        </div>

        <div className="section">
          <h3>播放</h3>
          <div className="card">
            <div className="card-row">
              <div className="grow">
                <div className="title">播放模式</div>
                <div className="detail">控制手动切歌与播完后的行为</div>
              </div>
              <button className="btn-secondary" type="button" onClick={onCyclePlaybackMode}>
                {PLAYBACK_MODE_LABEL[playbackMode]}
              </button>
            </div>
          </div>
        </div>

        <div className="section">
          <h3>快捷键</h3>
          <div className="card shortcuts">
            <div className="card-row"><span>播放 / 暂停</span><kbd>Space</kbd></div>
            <div className="card-row"><span>上一首 / 下一首</span><span><kbd>Shift</kbd> + <kbd>←</kbd> / <kbd>→</kbd></span></div>
            <div className="card-row"><span>聚焦搜索</span><kbd>/</kbd></div>
          </div>
        </div>
      </div>
    </section>
  );
};

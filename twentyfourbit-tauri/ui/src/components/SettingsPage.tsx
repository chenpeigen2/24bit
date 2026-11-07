import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { Settings } from "../lib/types";

interface Props {
  quality: string;
  onQuality: (q: string) => void;
  onToast: (text: string, kind?: "ok" | "err") => void;
  account: string | null;
  onAccount: (name: string) => void;
}

/** Quality, account and storage. A destructive action arms before it fires. */
export const SettingsPage = ({
  quality,
  onQuality,
  onToast,
  account,
  onAccount,
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
    <section className="page">
      <header className="page-head" data-tauri-drag-region>
        <h1>设置</h1>
        <p>音质、账号与存储</p>
      </header>

      <div className="settings">
        <div className="section">
          <h3>音质偏好</h3>
          <div className="card">
            {[
              ["master", "至臻母带", "24bit / 192kHz FLAC，含歌词"],
              ["surround", "高清环绕声", "较小体积，不含歌词"],
            ].map(([value, title, detail]) => (
              <div
                key={value}
                className={`card-row${quality === value ? " is-selected" : ""}`}
                onClick={() => {
                  onQuality(value as string);
                  void api.setQuality(value as string);
                }}
              >
                <div className="radio" />
                <div className="grow">
                  <div className="title">{title}</div>
                  <div className="detail">{detail}</div>
                </div>
              </div>
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
                  placeholder="账号"
                  value={user}
                  onChange={(e) => setUser(e.target.value)}
                />
                <input
                  className="text-input"
                  placeholder="密码"
                  type="password"
                  value={pass}
                  onChange={(e) => setPass(e.target.value)}
                />
                <button
                  className="btn-primary"
                  disabled={busy || !user || !pass}
                  onClick={async () => {
                    setBusy(true);
                    try {
                      const name = await api.login(user, pass);
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
      </div>
    </section>
  );
};

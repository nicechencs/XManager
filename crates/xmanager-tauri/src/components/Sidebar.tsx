import type { RouteView, UiSnapshot } from "../types";
import { store } from "../hooks";
import { checkForUpdate } from "../update";
import { Btn, CountBadge, StatusPill } from "./common";

export type ThemeMode = "light" | "dark";

const NAV_ITEMS: { route: RouteView; label: string; compact: string; glyph: string }[] = [
  { route: "library", label: "内容库", compact: "内容", glyph: "☰" },
  { route: "insights", label: "数据洞察", compact: "洞察", glyph: "▦" },
  { route: "cleanup", label: "安全清理", compact: "清理", glyph: "◉" },
];

export function Sidebar({
  snapshot,
  compact,
  themeMode,
  onThemeChange,
}: {
  snapshot: UiSnapshot;
  compact: boolean;
  themeMode: ThemeMode;
  onThemeChange: (mode: ThemeMode) => void;
}) {
  const synced = snapshot.last_synced_at;
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <img src="/logo.png" alt="XManager" />
        <span className="brand-name">XManager</span>
      </div>

      {NAV_ITEMS.map((item) => (
        <button
          key={item.route}
          className={`nav-item${snapshot.route === item.route ? " active" : ""}`}
          onClick={() => void store.dispatch({ type: "set_route", route: item.route })}
        >
          <span className="nav-glyph">{item.glyph}</span>
          <span className="nav-label">{compact ? item.compact : item.label}</span>
          {item.route === "cleanup" ? (
            <CountBadge n={snapshot.cleanup.count} />
          ) : null}
        </button>
      ))}

      <div className="sidebar-spacer" />

      <div className="account-card">
        <StatusPill healthy={snapshot.credentials.healthy} label={snapshot.credentials.msg} />
        <div className="account-meta">
          {compact
            ? `${snapshot.all_tweets.length} 条`
            : synced
              ? `${snapshot.all_tweets.length} 条 · 同步 ${synced}`
              : `${snapshot.all_tweets.length} 条 · 尚未同步`}
        </div>
        <Btn
          variant="ghost"
          disabled={snapshot.loading}
          onClick={() =>
            void checkForUpdate(true, (message) => {
              void store.dispatch({ type: "set_status", message });
            })
          }
        >
          {compact ? "更新" : "检查更新"}
        </Btn>
        <Btn
          variant="ghost"
          disabled={snapshot.loading}
          onClick={() => void store.refreshWhoami()}
        >
          {compact ? "刷新" : "刷新状态"}
        </Btn>
      </div>

      <div className="theme-switch">
        <button
          className={themeMode === "light" ? "active" : ""}
          onClick={() => onThemeChange("light")}
        >
          {compact ? "浅" : "浅色"}
        </button>
        <button
          className={themeMode === "dark" ? "active" : ""}
          onClick={() => onThemeChange("dark")}
        >
          {compact ? "深" : "深色"}
        </button>
      </div>
    </aside>
  );
}

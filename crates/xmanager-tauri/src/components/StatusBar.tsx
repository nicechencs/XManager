import type { UiSnapshot } from "../types";
import { StatusPill } from "./common";

export function StatusBar({
  snapshot,
  pending,
}: {
  snapshot: UiSnapshot;
  pending: string | null;
}) {
  const busy = snapshot.loading || pending !== null;
  return (
    <footer className="status-bar">
      <span>
        共 {snapshot.all_tweets.length} 条 · 筛选 {snapshot.filtered.length} ·
        已选 {snapshot.selected.length} · 候选 {snapshot.cleanup.count}
      </span>
      {busy ? <span className="status-dot">● 处理中…</span> : null}
      <span className="status-msg">{snapshot.status_msg}</span>
      <span className="status-hint">
        J/K 上下 · 空格勾选 · / 筛选 · 1–3 切页 · Esc 关闭检查器
      </span>
      <StatusPill healthy={snapshot.credentials.healthy} label={snapshot.credentials.msg} />
    </footer>
  );
}

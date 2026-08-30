import type { UiSnapshot } from "../../types";
import { store } from "../../hooks";
import {
  displayDate,
  engagement,
  engagementRate,
  formatRate,
  kindOf,
  likeRate,
  views,
} from "../../tweet";
import { Btn, KindBadge } from "../common";

function MetricTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric-tile">
      <span className="metric-label">{label}</span>
      <span className="metric-value">{value}</span>
    </div>
  );
}

export function Inspector({
  snapshot,
  onClose,
}: {
  snapshot: UiSnapshot;
  onClose: () => void;
}) {
  const focusedId = snapshot.focused_tweet_id;
  const focused = focusedId
    ? (snapshot.filtered.find((t) => t.id === focusedId) ?? null)
    : null;
  if (!focusedId || !focused) {
    return (
      <div className="inspector">
        <div className="inspector-header">
          <span>检查器</span>
        </div>
        <div className="empty-state">
          <div className="empty-mark">📄</div>
          <div className="empty-title">未选择推文</div>
          <div className="empty-detail">
            在内容库中选择一条推文，这里将显示完整内容与详情。
          </div>
        </div>
      </div>
    );
  }

  const candidate = snapshot.cleanup.rows.find((row) => row.id === focusedId);
  const snapshotOnly = candidate !== undefined && candidate.tweet === null;
  const inSelection = snapshot.selected.includes(focusedId);

  return (
    <div className="inspector">
      <div className="inspector-header">
        <span>推文检查</span>
        <Btn variant="ghost" onClick={onClose}>
          关闭
        </Btn>
      </div>
      <div className="inspector-body">
        <div className="chip-row">
          <KindBadge kind={kindOf(focused)} />
          {candidate !== undefined ? (
            <span className="kind-badge retweet">已在安全清理</span>
          ) : null}
          {snapshotOnly ? <span className="caption">不在当前筛选结果中，显示候选快照</span> : null}
        </div>

        <div className="inspector-text">{focused.text}</div>

        <div className="metric-grid">
          <MetricTile label="曝光" value={views(focused).toLocaleString("zh-CN")} />
          <MetricTile label="点赞" value={focused.public_metrics.like_count.toLocaleString("zh-CN")} />
          <MetricTile label="收藏" value={focused.public_metrics.bookmark_count.toLocaleString("zh-CN")} />
          <MetricTile label="转发" value={focused.public_metrics.retweet_count.toLocaleString("zh-CN")} />
          <MetricTile label="回复" value={focused.public_metrics.reply_count.toLocaleString("zh-CN")} />
          <MetricTile label="互动量" value={engagement(focused).toLocaleString("zh-CN")} />
          <MetricTile label="点赞率" value={formatRate(likeRate(focused))} />
          <MetricTile label="互动率" value={formatRate(engagementRate(focused))} />
        </div>

        <div className="context-row">
          <span>发布时间</span>
          <span className="context-value">{displayDate(focused)}</span>
        </div>
        <div className="context-row">
          <span>推文 ID</span>
          <span className="context-value mono">{focused.id}</span>
        </div>

        <div className="chip-row">
          <Btn variant="ghost" onClick={() => void store.dispatch({ type: "focus_prev" })}>
            上一条
          </Btn>
          <Btn variant="ghost" onClick={() => void store.dispatch({ type: "focus_next" })}>
            下一条
          </Btn>
        </div>

        <div className="chip-row">
          <Btn variant="danger" onClick={() => void store.dispatch({ type: "request_library_delete" })}>
            删除选中
          </Btn>
          {candidate !== undefined ? (
            <Btn
              variant="ghost"
              onClick={() => void store.dispatch({ type: "remove_cleanup_candidate", id: focusedId })}
            >
              移出安全清理
            </Btn>
          ) : (
            <Btn
              variant="ghost"
              onClick={() => void store.dispatch({ type: "add_cleanup_candidate", id: focusedId })}
            >
              加入安全清理
            </Btn>
          )}
        </div>

        <div className="caption">已选 {snapshot.selected.length} 条{inSelection ? " · 含当前推文" : ""}</div>
      </div>
    </div>
  );
}

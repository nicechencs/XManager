import type { UiSnapshot } from "../../types";
import { store } from "../../hooks";
import {
  displayDate,
  kindOf,
  KIND_LABEL_ZH,
  truncateText,
  views,
} from "../../tweet";
import { Btn, CountBadge, EmptyMark, PageHeading } from "../common";

/** Excerpt label for preview/delete samples: text first, raw id otherwise. */
function previewLabel(snapshot: UiSnapshot, id: string): string {
  const fromAll = snapshot.all_tweets.find((t) => t.id === id);
  const text = fromAll?.text ?? null;
  if (text === null) return id;
  const flat = text.replace(/\n/g, " ");
  const excerpt = Array.from(flat).slice(0, 36).join("");
  const ellipsis = Array.from(flat).length > 36 ? "…" : "";
  return `${excerpt}${ellipsis}`;
}

function sampleList(snapshot: UiSnapshot, ids: string[], limit: number): string {
  return ids
    .slice(0, limit)
    .map((id) => previewLabel(snapshot, id))
    .join("、");
}

export function CleanupPage({ snapshot }: { snapshot: UiSnapshot }) {
  const cleanup = snapshot.cleanup;
  const confirm = cleanup.delete_confirm;
  const cleanupConfirm = confirm !== null && confirm.source === "cleanup" ? confirm : null;
  const preview = cleanup.preview_outcome;
  const outcome = cleanup.last_delete_outcome;

  return (
    <div className="page">
      <PageHeading title="安全清理" subtitle={cleanup.next_hint} />

      <div className="panel cleanup-counter">
        <span className="cleanup-count-line">
          {cleanup.count} 条候选 <CountBadge n={cleanup.count} />
        </span>
        <span style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
          <Btn
            variant="danger"
            disabled={snapshot.loading || cleanup.count === 0}
            onClick={() => void store.deletePreviewed()}
          >
            删除 {cleanup.count} 条
          </Btn>
          <Btn
            variant="ghost"
            disabled={snapshot.loading || cleanup.count === 0}
            onClick={() => void store.dispatch({ type: "request_clear_cleanup" })}
          >
            {cleanup.clear_armed ? "确认清空" : "清空候选"}
          </Btn>
        </span>
      </div>

      <div className="panel">
        <span className="caption">删除会自动备份到导出目录。需要时也可另存一份。</span>
        <div className="chip-row">
          <Btn
            variant="ghost"
            disabled={snapshot.loading || cleanup.count === 0}
            onClick={() => void store.export("cleanup", "csv")}
          >
            另存 CSV
          </Btn>
          <Btn
            variant="ghost"
            disabled={snapshot.loading || cleanup.count === 0}
            onClick={() => void store.export("cleanup", "json")}
          >
            另存 JSON
          </Btn>
          {cleanup.has_valid_backup ? <span className="outcome-line ok">✓ 当前候选已备份</span> : null}
          {cleanup.has_valid_preview ? <span className="outcome-line ok">✓ 已预演</span> : null}
        </div>
      </div>

      {preview !== null ? (
        <div className="panel">
          <span className="outcome-headline">{previewSummaryLine(preview.total, preview)}</span>
          <div className="outcome-line ok">
            可删除 {preview.deletable.length} 条：{sampleList(snapshot, preview.deletable, 6)}
          </div>
          <div className="outcome-line warn">
            不在当前库 {preview.missing.length} 条：{sampleList(snapshot, preview.missing, 6)}
          </div>
          <div className="outcome-line err">
            失败 {preview.failed.length} 条：
            {preview.failed.slice(0, 3).map(([id, err]) => `${previewLabel(snapshot, id)}（${err}）`).join("；")}
          </div>
        </div>
      ) : null}

      {cleanupConfirm !== null ? (
        <div className="banner danger" style={{ padding: 16 }}>
          <span className="banner-text">
            <div className="outcome-headline">{cleanupConfirm.prompt}</div>
            <div className="caption">{cleanupConfirm.caption}</div>
          </span>
          <span className="banner-actions">
            <Btn variant="danger" onClick={() => void store.confirmAndDelete()}>
              确认删除 {cleanupConfirm.expected_count} 条
            </Btn>
            <Btn variant="ghost" onClick={() => void store.dispatch({ type: "cancel_delete_confirm" })}>
              取消
            </Btn>
          </span>
        </div>
      ) : null}

      {outcome !== null ? (
        <div className="panel">
          <span className="outcome-headline">{outcomeSummaryLine(outcome)}</span>
          <div className="outcome-line ok">
            成功 {outcome.succeeded.length} 条：
            {sampleList(snapshot, outcome.succeeded, 5)}
          </div>
          {outcome.failed.length > 0 ? (
            <div className="outcome-line err">
              仍留在候选中的失败项 {outcome.failed.length} 条：
              {outcome.failed
                .slice(0, 5)
                .map(([id, err]) => `${previewLabel(snapshot, id)}（${err}）`)
                .join("；")}
            </div>
          ) : null}
        </div>
      ) : null}

      {cleanup.rows.length === 0 ? (
        <div className="panel" style={{ flex: 1 }}>
          <div className="empty-state">
            <EmptyMark glyph="◉" />
            <div className="empty-title">还没有候选推文</div>
            <div className="empty-detail">
              内容库可直接删除选中。需要复核时再加入安全清理。
            </div>
            <Btn
              variant="primary"
              onClick={() => void store.dispatch({ type: "set_route", route: "library" })}
            >
              返回内容库选择
            </Btn>
          </div>
        </div>
      ) : (
        cleanup.rows.map((row) => {
          const tweet = row.tweet;
          const meta = tweet
            ? `${KIND_LABEL_ZH[kindOf(tweet)]} · 曝光 ${views(tweet)} · ${displayDate(tweet)}`
            : "快照 · 详情未知";
          return (
            <div className="candidate-row" key={row.id}>
              {tweet ? (
                <span className={`kind-badge ${kindOf(tweet)}`}>
                  {KIND_LABEL_ZH[kindOf(tweet)]}
                </span>
              ) : (
                <span className="snapshot-badge">快照</span>
              )}
              <div className="candidate-main">
                <span className="candidate-preview">{truncateText(tweet?.text ?? row.preview, 90)}</span>
                <span className="candidate-meta">{meta}</span>
              </div>
              <Btn
                variant="ghost"
                onClick={() =>
                  void store.dispatch({ type: "open_tweet_in_library", id: row.id })
                }
              >
                查看
              </Btn>
              <Btn
                variant="ghost"
                onClick={() =>
                  void store.dispatch({ type: "remove_cleanup_candidate", id: row.id })
                }
              >
                移除
              </Btn>
            </div>
          );
        })
      )}
    </div>
  );
}

function previewSummaryLine(total: number, preview: UiSnapshot["cleanup"]["preview_outcome"]): string {
  if (preview === null) return "";
  return `预演完成：可删 ${preview.deletable.length} · 不在库 ${preview.missing.length} · 失败 ${preview.failed.length}（共 ${total}）`;
}

function outcomeSummaryLine(outcome: UiSnapshot["cleanup"]["last_delete_outcome"]): string {
  if (outcome === null) return "";
  return `删除结果：成功 ${outcome.succeeded.length} · 失败 ${outcome.failed.length}（共 ${outcome.succeeded.length + outcome.failed.length}）`;
}

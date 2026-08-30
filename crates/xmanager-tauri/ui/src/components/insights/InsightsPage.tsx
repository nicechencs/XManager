import type { TimeRange, UiSnapshot } from "../../types";
import { store } from "../../hooks";
import { KIND_SHORT_ZH, kindOf, truncateText, views } from "../../tweet";
import { Btn, Chip, EmptyMark, PageHeading, formatNumber } from "../common";

/** 5 lowest-view tweets of the full synced set (GPUI lowest_view_samples). */
function lowestViewSamples(snapshot: UiSnapshot) {
  return [...snapshot.all_tweets]
    .sort((a, b) => views(a) - views(b) || a.id.localeCompare(b.id))
    .slice(0, 5);
}

function activeFilterSummary(snapshot: UiSnapshot): string {
  const labels: Record<string, string> = {
    Views: "曝光",
    LikeRate: "点赞率",
    BookmarkRate: "收藏率",
    EngagementRate: "互动率",
    RetweetRate: "转发率",
    ReplyRate: "回复率",
    Engagement: "互动量",
    Date: "日期",
  };
  const parts: string[] = [];
  const f = snapshot.applied_filter;
  if (f.time_range !== "All") parts.push(rangeLabel(f.time_range));
  if (f.max_views !== null) parts.push(`曝光≤${f.max_views}`);
  if (f.min_views !== null) parts.push(`曝光≥${f.min_views}`);
  if (f.max_engagement !== null) parts.push(`互动≤${f.max_engagement}`);
  if (f.older_than_days !== null) parts.push(`早于${f.older_than_days}天`);
  if (f.top_n !== null) parts.push(`Top${f.top_n}`);
  parts.push(`排序 ${labels[f.sort] ?? f.sort} ${f.order === "Asc" ? "最低优先" : "最高优先"}`);
  return parts.join(" · ");
}

function rangeLabel(range: TimeRange): string {
  const labels: Record<string, string> = {
    All: "全部时间",
    Hours24: "24 小时",
    Days7: "7 天",
    Days30: "30 天",
    Days90: "90 天",
    Days180: "180 天",
    Days360: "360 天",
  };
  return labels[range] ?? range;
}

function ScopeCard({
  snapshot,
  current,
}: {
  snapshot: UiSnapshot;
  current: boolean;
}) {
  const summary = current ? snapshot.summary : snapshot.all_summary;
  const pct =
    snapshot.all_summary.count > 0
      ? (summary.count / snapshot.all_summary.count) * 100
      : 0;
  return (
    <div className={`scope-card${current ? " current" : ""}`}>
      <span className="scope-title">
        {current ? "当前切片" : "全部"} {formatNumber(summary.count)} 条
      </span>
      {current ? (
        <span className="caption">占全部 {pct.toFixed(0)}%</span>
      ) : (
        <span className="caption">全部已同步数据</span>
      )}
      <div className="metric-grid" style={{ gridTemplateColumns: "repeat(3, 1fr)" }}>
        <div className="metric-tile">
          <span className="metric-label">均曝光</span>
          <span className="metric-value">{formatNumber(Math.round(summary.avg_views))}</span>
        </div>
        <div className="metric-tile">
          <span className="metric-label">中位曝光</span>
          <span className="metric-value">{formatNumber(Math.round(summary.median_views))}</span>
        </div>
        <div className="metric-tile">
          <span className="metric-label">均互率</span>
          <span className="metric-value">{(summary.avg_engagement_rate * 100).toFixed(2)}%</span>
        </div>
      </div>
      <div className="kind-mix">
        原{summary.original_count} · 引{summary.quote_count} · 回{summary.reply_count} · 转
        {summary.retweet_count}
      </div>
    </div>
  );
}

export function InsightsPage({ snapshot }: { snapshot: UiSnapshot }) {
  const samples = lowestViewSamples(snapshot);
  const maxCount = Math.max(1, ...snapshot.histogram.map((b) => b.count));

  return (
    <div className="page">
      <PageHeading
        title="数据洞察"
        subtitle="看全部已同步内容的形状，当前内容库筛选是高亮切片。"
        actions={
          <>
            <Btn
              variant="ghost"
              disabled={snapshot.loading || snapshot.filtered.length === 0}
              onClick={() => void store.export("library", "csv")}
            >
              导出 CSV
            </Btn>
            <Btn
              variant="ghost"
              disabled={snapshot.loading || snapshot.filtered.length === 0}
              onClick={() => void store.export("library", "json")}
            >
              导出 JSON
            </Btn>
          </>
        }
      />

      {snapshot.all_tweets.length === 0 ? (
        <div className="panel" style={{ flex: 1 }}>
          <div className="empty-state">
            <EmptyMark glyph="▦" />
            <div className="empty-title">还没有可分析的数据</div>
            <div className="empty-detail">
              先到内容库拉取推文，这里会显示全部数据的曝光分布。
            </div>
          </div>
        </div>
      ) : (
        <>
          <div className="panel filter-card">
            <span className="caption">
              当前切片 {snapshot.filtered.length}/{snapshot.all_tweets.length} 条 ·{" "}
              {activeFilterSummary(snapshot)}
            </span>
            <span style={{ marginLeft: "auto" }}>
              <Btn
                variant="ghost"
                onClick={() => void store.dispatch({ type: "set_route", route: "library" })}
              >
                在内容库查看
              </Btn>
            </span>
          </div>

          <div className="scope-cards">
            <ScopeCard snapshot={snapshot} current={false} />
            <ScopeCard snapshot={snapshot} current={true} />
          </div>

          <div className="panel">
            <span className="section-label">曝光分布（全部数据，点柱去内容库）</span>
            <div className="histogram">
              {snapshot.histogram.map((bucket) => (
                <div
                  key={bucket.label}
                  className={`hist-row${bucket.overlaps ? " current" : ""}`}
                >
                  <span className="hist-label">{bucket.label}</span>
                  <div
                    className="hist-track"
                    title={bucket.overlaps ? "当前切片" : "查看"}
                    onClick={() =>
                      void store.dispatch({
                        type: "apply_histogram_bucket",
                        label: bucket.label,
                      })
                    }
                  >
                    <div
                      className="hist-bar"
                      style={{ width: `${(bucket.count / maxCount) * 100}%` }}
                    />
                  </div>
                  <span className="hist-count">{bucket.count}</span>
                  <span className="hist-caption">
                    {bucket.overlaps ? "当前切片" : bucket.count > 0 ? "查看" : ""}
                  </span>
                </div>
              ))}
            </div>
          </div>

          <div className="panel">
            <span className="section-label">常用切片（应用到内容库）</span>
            <div className="preset-row">
              <Chip
                onClick={() =>
                  void store.dispatch({ type: "apply_low_exposure", threshold: 10 })
                }
              >
                ≤10 曝光
              </Chip>
              <Chip
                onClick={() =>
                  void store.dispatch({ type: "apply_low_exposure", threshold: 20 })
                }
              >
                ≤20 曝光
              </Chip>
              <Chip
                onClick={() =>
                  void store.dispatch({ type: "apply_low_exposure", threshold: 50 })
                }
              >
                ≤50 曝光待清理
              </Chip>
              <Chip
                onClick={() =>
                  void store.dispatch({ type: "apply_low_exposure", threshold: 100 })
                }
              >
                ≤100 曝光
              </Chip>
              <Chip
                onClick={() =>
                  void store.dispatch({
                    type: "apply_rank_preset",
                    range: "Days30",
                    sort: "Views",
                    order: "Asc",
                    top_n: 50,
                  })
                }
              >
                30天 · 曝光最低
              </Chip>
              <Chip
                onClick={() =>
                  void store.dispatch({
                    type: "apply_rank_preset",
                    range: "Days7",
                    sort: "LikeRate",
                    order: "Desc",
                    top_n: 20,
                  })
                }
              >
                7天 · 点赞率最高
              </Chip>
              <Chip
                onClick={() =>
                  void store.dispatch({
                    type: "apply_rank_preset",
                    range: "Days30",
                    sort: "BookmarkRate",
                    order: "Desc",
                    top_n: 20,
                  })
                }
              >
                30天 · 收藏率最高
              </Chip>
            </div>
          </div>

          <div className="panel">
            <span className="section-label">低曝光样本 · 点一行去内容库</span>
            {samples.map((tweet) => (
              <div
                key={tweet.id}
                className="sample-row"
                onClick={() =>
                  void store.dispatch({ type: "open_tweet_in_library", id: tweet.id })
                }
              >
                <span className={`kind-badge ${kindOf(tweet)}`}>
                  {KIND_SHORT_ZH[kindOf(tweet)]}
                </span>
                <span className="sample-preview">{truncateText(tweet.text, 72)}</span>
                <span className="sample-views">曝光 {views(tweet)}</span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

import { useRef } from "react";
import type { LayoutMode } from "../../hooks";
import { store } from "../../hooks";
import type { AppliedFilterChip, UiSnapshot } from "../../types";
import { Btn, Chip, EmptyMark, PageHeading, RemovableChip } from "../common";
import { FilterDrawer } from "./FilterDrawer";
import { Inspector } from "./Inspector";
import { TweetList } from "./TweetList";

function chipLabel(chip: AppliedFilterChip): string {
  switch (chip.chip) {
    case "time_range":
      return chip.label;
    case "max_views":
      return `曝光 ≤ ${chip.value}`;
    case "min_views":
      return `曝光 ≥ ${chip.value}`;
    case "max_engagement":
      return `互动 ≤ ${chip.value}`;
    case "older_than_days":
      return `早于 ${chip.days} 天`;
    case "top_n":
      return `Top ${chip.n}`;
    case "sort":
      return `排序：${chip.label}`;
    case "order":
      return chip.label;
    case "kinds":
      return chip.label;
  }
}

function SkeletonRows() {
  const widths = ["60%", "42%", "78%", "36%", "55%", "68%", "30%", "48%"];
  return (
    <div style={{ flex: 1, overflow: "hidden" }}>
      {widths.map((width, index) => (
        <div className="skeleton-row" key={index}>
          <span className="skeleton-bar" style={{ width: 16 }} />
          <span className="skeleton-bar" style={{ width: 56 }} />
          <span className="skeleton-bar" style={{ width }} />
          <span className="skeleton-bar" style={{ width: 40 }} />
          <span className="skeleton-bar" style={{ width: 48 }} />
        </div>
      ))}
    </div>
  );
}

function EmptyStatePanel({ snapshot }: { snapshot: UiSnapshot }) {
  const empty = snapshot.empty_state;
  const action = (() => {
    switch (empty.kind) {
      case "loading":
        return null;
      case "credentials_missing":
      case "credentials_swapped":
        return (
          <Btn variant="primary" disabled={snapshot.loading} onClick={() => void store.refreshWhoami()}>
            刷新状态
          </Btn>
        );
      case "fetch_failed":
      case "never_synced":
      case "account_empty":
        return (
          <Btn variant="primary" disabled={snapshot.loading} onClick={() => void store.fetchTweets()}>
            拉取并分析
          </Btn>
        );
      case "filtered_empty":
        return (
          <>
            <div className="chip-row">
              {snapshot.applied_chips.map((chip, index) => (
                <RemovableChip
                  key={index}
                  label={chipLabel(chip)}
                  onRemove={() => void store.dispatch({ type: "remove_chip", chip })}
                />
              ))}
            </div>
            <Btn variant="primary" onClick={() => void store.dispatch({ type: "clear_filters" })}>
              清除筛选
            </Btn>
          </>
        );
    }
  })();

  return (
    <div className="empty-state">
      <EmptyMark glyph={empty.mark} />
      <div className="empty-title">{empty.title}</div>
      <div className="empty-detail">{empty.detail}</div>
      {action}
    </div>
  );
}

export function LibraryPage({
  snapshot,
  layout,
  drawerOpen,
  inspectorWidth,
  onInspectorResize,
  onInspectorReset,
}: {
  snapshot: UiSnapshot;
  layout: LayoutMode;
  drawerOpen: boolean;
  inspectorWidth: number;
  onInspectorResize: (width: number) => void;
  onInspectorReset: () => void;
}) {
  const chips = snapshot.applied_chips;
  const filterCount = chips.length;
  const confirm = snapshot.cleanup.delete_confirm;
  const libraryConfirm = confirm !== null && confirm.source === "library" ? confirm : null;

  const showDrawerColumn = drawerOpen && layout !== "narrow";
  const showInspectorColumn =
    layout === "wide" && snapshot.focused_tweet_id !== null;

  return (
    <div className="page">
      <PageHeading
        title="内容库"
        subtitle="筛选、检查并整理你的 X 内容"
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
            <Btn
              variant="primary"
              disabled={snapshot.loading}
              onClick={() => void store.fetchTweets()}
            >
              {snapshot.loading ? "分析中…" : "拉取并分析"}
            </Btn>
          </>
        }
      />

      <div className="panel filter-card">
        <Chip
          active={drawerOpen}
          onClick={() => store.toggleFilterDrawer()}
        >
          {drawerOpen ? `收起筛选 · ${filterCount}` : `筛选 · ${filterCount}`}
        </Chip>
        <Chip active={false} onClick={() => store.openDrawer()}>
          排序：{sortLabelZh(snapshot)}
        </Chip>
        {chips.map((chip, index) => (
          <RemovableChip
            key={index}
            label={chipLabel(chip)}
            onRemove={() => void store.dispatch({ type: "remove_chip", chip })}
          />
        ))}
        {filterCount > 0 ? (
          <Btn variant="ghost" onClick={() => void store.dispatch({ type: "clear_filters" })}>
            清除全部
          </Btn>
        ) : null}
        <div
          className="slice-stats"
          onClick={() => void store.dispatch({ type: "set_route", route: "insights" })}
          title="查看数据洞察"
        >
          <div className="caption">当前切片 · 点此看洞察</div>
          <div className="caption">{snapshot.slice_stats_line}</div>
        </div>
      </div>

      {snapshot.cleanup.notice !== null ? (
        <div className="banner accent">
          <span className="banner-text">{snapshot.cleanup.notice}</span>
          <span className="banner-actions">
            <Btn
              variant="primary"
              onClick={() => void store.dispatch({ type: "set_route", route: "cleanup" })}
            >
              去安全清理
            </Btn>
            <Btn variant="ghost" onClick={() => void store.dispatch({ type: "dismiss_cleanup_notice" })}>
              继续挑选
            </Btn>
          </span>
        </div>
      ) : null}

      {libraryConfirm !== null ? (
        <div className="banner danger">
          <span className="banner-text">
            <div className="outcome-headline">{libraryConfirm.prompt}</div>
            <div className="caption">{libraryConfirm.caption}</div>
          </span>
          <span className="banner-actions">
            <Btn variant="danger" onClick={() => void store.confirmAndDelete()}>
              确认
            </Btn>
            <Btn variant="ghost" onClick={() => void store.dispatch({ type: "cancel_delete_confirm" })}>
              取消
            </Btn>
          </span>
        </div>
      ) : null}

      {snapshot.loading ? (
        <div className="banner accent">
          <span className="banner-text">
            {snapshot.all_tweets.length === 0
              ? "首次加载中，请稍候…"
              : "正在更新…已保留当前列表，完成后自动刷新。"}
          </span>
          <span className="caption">{snapshot.status_msg}</span>
        </div>
      ) : null}

      <div className="library-main">
        {showDrawerColumn ? <FilterDrawer snapshot={snapshot} /> : null}

        <div className="library-list-col">
          {snapshot.all_tweets.length === 0 ? (
            snapshot.loading ? (
              <SkeletonRows />
            ) : (
              <EmptyStatePanel snapshot={snapshot} />
            )
          ) : snapshot.filtered.length === 0 ? (
            <EmptyStatePanel snapshot={snapshot} />
          ) : (
            <>
              <TweetList snapshot={snapshot} />
              <BulkBar snapshot={snapshot} />
            </>
          )}
        </div>

        {showInspectorColumn ? (
          <>
            <Splitter
              onDrag={(delta) => onInspectorResize(delta)}
              onReset={onInspectorReset}
            />
            <div style={{ width: inspectorWidth, flex: "none", display: "flex" }}>
              <Inspector
                snapshot={snapshot}
                onClose={() => void store.dispatch({ type: "unfocus_tweet" })}
              />
            </div>
          </>
        ) : null}
      </div>

      {layout === "narrow" && snapshot.focused_tweet_id !== null ? (
        <div className="overlay-layer">
          <Inspector snapshot={snapshot} onClose={() => void store.dispatch({ type: "unfocus_tweet" })} />
        </div>
      ) : null}
      {layout === "medium" && snapshot.focused_tweet_id !== null ? (
        <div className="overlay-layer">
          <Inspector snapshot={snapshot} onClose={() => void store.dispatch({ type: "unfocus_tweet" })} />
        </div>
      ) : null}
      {layout === "narrow" && drawerOpen ? (
        <div className="overlay-layer">
          <FilterDrawerWithClose snapshot={snapshot} />
        </div>
      ) : null}
    </div>
  );
}

function FilterDrawerWithClose({ snapshot }: { snapshot: UiSnapshot }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", width: "100%" }}>
      <div className="drawer-header" style={{ padding: 16 }}>
        <span className="drawer-title">筛选与排序</span>
        <Btn variant="ghost" onClick={() => store.closeDrawer()}>
          关闭
        </Btn>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        <FilterDrawer snapshot={snapshot} />
      </div>
    </div>
  );
}

function BulkBar({ snapshot }: { snapshot: UiSnapshot }) {
  const staged =
    snapshot.selected.length > 0
      ? `已选 ${snapshot.selected.length} 条`
      : snapshot.focused_tweet_id !== null
        ? "已查看当前推文"
        : "已选 0 条";
  const canStage = snapshot.selected.length > 0 || snapshot.focused_tweet_id !== null;
  return (
    <div className="bulk-bar">
      <span className="bulk-count">{staged}</span>
      <Btn
        variant="ghost"
        disabled={snapshot.loading || snapshot.filtered.length === 0}
        onClick={() => void store.dispatch({ type: "select_all" })}
      >
        全选
      </Btn>
      <Btn
        variant="ghost"
        disabled={snapshot.loading || snapshot.selected.length === 0}
        onClick={() => void store.dispatch({ type: "select_none" })}
      >
        清除选择
      </Btn>
      <Btn
        variant="ghost"
        disabled={snapshot.loading || snapshot.filtered.length === 0}
        onClick={() => void store.dispatch({ type: "invert_selection" })}
      >
        反选
      </Btn>
      <Btn
        variant="danger"
        disabled={snapshot.loading || !canStage}
        onClick={() => void store.dispatch({ type: "request_library_delete" })}
      >
        删除选中
      </Btn>
      <Btn
        variant="ghost"
        disabled={snapshot.loading || !canStage}
        onClick={() => void store.dispatch({ type: "add_selected_to_cleanup" })}
      >
        加入安全清理
      </Btn>
    </div>
  );
}

function Splitter({
  onDrag,
  onReset,
}: {
  onDrag: (deltaX: number) => void;
  onReset: () => void;
}) {
  const drag = useRef<{ lastX: number } | null>(null);
  return (
    <div
      className="splitter"
      title="拖动调整检查器宽度 · 双击复位"
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = { lastX: event.clientX };
      }}
      onPointerMove={(event) => {
        if (drag.current === null || !(event.buttons & 1)) return;
        onDrag(event.clientX - drag.current.lastX);
        drag.current.lastX = event.clientX;
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
      onDoubleClick={onReset}
    />
  );
}

function sortLabelZh(snapshot: UiSnapshot): string {
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
  return labels[snapshot.applied_filter.sort] ?? snapshot.applied_filter.sort;
}

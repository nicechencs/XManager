import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { SortField, Tweet, UiSnapshot } from "../../types";
import { store } from "../../hooks";
import { displayDateShort, kindOf, truncateText, views } from "../../tweet";
import { CheckboxMark, KindBadge } from "../common";

const ROW_H = 56;

const COL = { check: 40, kind: 64, date: 80, views: 80 } as const;

function sortLabel(field: SortField, snapshot: UiSnapshot): string {
  const base = field === "Date" ? "日期" : "曝光";
  const active = snapshot.applied_filter.sort === field;
  if (!active) return base;
  return `${base} ${snapshot.applied_filter.order === "Asc" ? "↑" : "↓"}`;
}

function TweetRow({
  tweet,
  index,
  start,
  snapshot,
}: {
  tweet: Tweet;
  index: number;
  start: number;
  snapshot: UiSnapshot;
}) {
  const selected = snapshot.selected.includes(tweet.id);
  const focused = snapshot.focused_tweet_id === tweet.id;
  const zebra = index % 2 === 0 ? "row-zebra" : "row-zebra-alt";
  const stateClass = focused ? "row-focused" : selected ? "row-selected" : zebra;
  return (
    <div
      className={`tweet-row ${stateClass}`}
      style={{ transform: `translateY(${start}px)`, height: ROW_H }}
      onClick={() => void store.dispatch({ type: "toggle_tweet_focus", id: tweet.id })}
    >
      <div
        className="cell check"
        onClick={(event) => {
          event.stopPropagation();
          void store.dispatch({ type: "toggle_selected", id: tweet.id });
        }}
      >
        <CheckboxMark checked={selected} />
      </div>
      <div className="cell kind">
        <KindBadge kind={kindOf(tweet)} />
      </div>
      <div className="cell" style={{ flex: 1, minWidth: 0 }}>
        <span className="content-preview">{truncateText(tweet.text, 180)}</span>
      </div>
      <div className="cell date">{displayDateShort(tweet)}</div>
      <div className="cell views">{views(tweet).toLocaleString("zh-CN")}</div>
    </div>
  );
}

export function TweetList({ snapshot }: { snapshot: UiSnapshot }) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const rows = snapshot.filtered;

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => ROW_H,
    overscan: 12,
  });

  if (rows.length === 0) return null;

  return (
    <div className="tweet-table">
      <div className="tweet-header">
        <div className="th" style={{ width: COL.check }} />
        <div className="th" style={{ width: COL.kind, justifyContent: "center" }}>
          类型
        </div>
        <div className="th" style={{ flex: 1, minWidth: 0 }}>
          内容
        </div>
        <div
          className="th sortable"
          style={{ width: COL.date }}
          onClick={() => void store.dispatch({ type: "sort_header", field: "Date" })}
        >
          {sortLabel("Date", snapshot)}
        </div>
        <div
          className="th sortable"
          style={{ width: COL.views }}
          onClick={() => void store.dispatch({ type: "sort_header", field: "Views" })}
        >
          {sortLabel("Views", snapshot)}
        </div>
      </div>
      <div className="tweet-viewport" ref={viewportRef}>
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((item) => {
            const tweet = rows[item.index];
            return (
              <TweetRow
                key={tweet.id}
                tweet={tweet}
                index={item.index}
                start={item.start}
                snapshot={snapshot}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}

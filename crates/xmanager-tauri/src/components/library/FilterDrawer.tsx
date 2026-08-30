import type { TimeRange, UiSnapshot } from "../../types";
import { store } from "../../hooks";
import { Btn, Chip, SectionLabel, Stepper } from "../common";

const TIME_PRESETS: { value: TimeRange; label: string }[] = [
  { value: "All", label: "全部时间" },
  { value: "Hours24", label: "24 小时" },
  { value: "Days7", label: "7 天" },
  { value: "Days30", label: "30 天" },
  { value: "Days90", label: "90 天" },
  { value: "Days180", label: "180 天" },
  { value: "Days360", label: "360 天" },
];

const KIND_CHIPS: { kind: "original" | "reply" | "retweet" | "quote"; label: string }[] = [
  { kind: "original", label: "原创" },
  { kind: "reply", label: "回帖" },
  { kind: "retweet", label: "转发" },
  { kind: "quote", label: "引用" },
];

const SORT_CHIPS: { value: UiSnapshot["filter_draft"]["sort"]; label: string }[] = [
  { value: "Views", label: "曝光" },
  { value: "LikeRate", label: "点赞率" },
  { value: "BookmarkRate", label: "收藏率" },
  { value: "EngagementRate", label: "互动率" },
  { value: "RetweetRate", label: "转发率" },
  { value: "ReplyRate", label: "回复率" },
  { value: "Engagement", label: "互动量" },
  { value: "Date", label: "日期" },
];

const TOP_N_CHIPS: { value: number | null; label: string }[] = [
  { value: null, label: "全部" },
  { value: 10, label: "Top 10" },
  { value: 20, label: "Top 20" },
  { value: 50, label: "Top 50" },
  { value: 100, label: "Top 100" },
];

const FETCH_LIMITS = [50, 100, 200, 500, 1000];

const LOW_EXPOSURE = [10, 20, 50, 100];

export function FilterDrawer({ snapshot }: { snapshot: UiSnapshot }) {
  const draft = snapshot.filter_draft;
  return (
    <div className="filter-drawer">
      <div className="drawer-header">
        <span className="drawer-title">筛选与排序</span>
      </div>
      <div className="caption">
        默认先看曝光≤50 的原创/引用，方便找低曝光内容。点芯片或「清除筛选」可看全部。改条件后列表立即更新。
      </div>

      <SectionLabel>时间范围</SectionLabel>
      <div className="chip-row">
        {TIME_PRESETS.map((preset) => (
          <Chip
            key={preset.value}
            active={draft.time_range === preset.value}
            onClick={() =>
              void store.dispatch({ type: "set_time_range", range: preset.value })
            }
          >
            {preset.label}
          </Chip>
        ))}
      </div>

      <SectionLabel>帖子类型</SectionLabel>
      <div className="chip-row">
        {KIND_CHIPS.map((item) => (
          <Chip
            key={item.kind}
            active={draft.kinds[item.kind]}
            onClick={() => void store.dispatch({ type: "toggle_kind", kind: item.kind })}
          >
            {item.label}
          </Chip>
        ))}
      </div>

      <SectionLabel>曝光上限</SectionLabel>
      <Stepper
        value={draft.max_views}
        min={0}
        max={100000}
        step={5}
        onChange={(value) =>
          void store.dispatch({ type: "set_stepper", field: "max_views", value })
        }
      />

      <SectionLabel>曝光下限</SectionLabel>
      <Stepper
        value={draft.min_views}
        min={0}
        max={100000}
        step={5}
        onChange={(value) =>
          void store.dispatch({ type: "set_stepper", field: "min_views", value })
        }
      />

      <SectionLabel>互动上限</SectionLabel>
      <Stepper
        value={draft.max_engagement}
        min={0}
        max={1000000}
        step={5}
        onChange={(value) =>
          void store.dispatch({ type: "set_stepper", field: "max_engagement", value })
        }
      />

      <SectionLabel>早于天数</SectionLabel>
      <Stepper
        value={draft.older_than_days}
        min={0}
        max={3600}
        step={1}
        onChange={(value) =>
          void store.dispatch({ type: "set_stepper", field: "older_than_days", value })
        }
      />

      <SectionLabel>排序指标</SectionLabel>
      <div className="chip-row">
        {SORT_CHIPS.map((item) => (
          <Chip
            key={item.value}
            active={draft.sort === item.value}
            onClick={() => void store.dispatch({ type: "set_sort", field: item.value })}
          >
            {item.label}
          </Chip>
        ))}
      </div>

      <SectionLabel>升降序</SectionLabel>
      <div className="chip-row">
        <Chip
          active={draft.order === "Asc"}
          onClick={() => void store.dispatch({ type: "set_order", order: "Asc" })}
        >
          最低优先
        </Chip>
        <Chip
          active={draft.order === "Desc"}
          onClick={() => void store.dispatch({ type: "set_order", order: "Desc" })}
        >
          最高优先
        </Chip>
      </div>

      <SectionLabel>结果数量</SectionLabel>
      <div className="chip-row">
        {TOP_N_CHIPS.map((item) => (
          <Chip
            key={item.label}
            active={draft.top_n === item.value}
            onClick={() => void store.dispatch({ type: "set_top_n", n: item.value })}
          >
            {item.label}
          </Chip>
        ))}
      </div>

      <SectionLabel>低曝光快捷</SectionLabel>
      <div className="chip-row">
        {LOW_EXPOSURE.map((threshold) => (
          <Chip
            key={threshold}
            onClick={() => void store.dispatch({ type: "apply_low_exposure", threshold })}
          >
            ≤{threshold}
          </Chip>
        ))}
      </div>

      <div className="chip-row">
        <Btn variant="primary" onClick={() => store.closeDrawer()}>
          完成
        </Btn>
        <Btn
          variant="ghost"
          onClick={() => {
            void store.dispatch({ type: "clear_filters" });
            store.closeDrawer();
          }}
        >
          清除筛选
        </Btn>
      </div>

      <SectionLabel>下次拉取（不改当前列表）</SectionLabel>
      <div className="caption">改完后请到内容库点「拉取并分析」。</div>
      <div className="chip-row">
        {FETCH_LIMITS.map((limit) => (
          <Chip
            key={limit}
            active={snapshot.fetch_limit === limit}
            onClick={() => void store.dispatch({ type: "set_fetch_limit", limit })}
          >
            {limit} 条
          </Chip>
        ))}
        <Chip
          active={draft.include_retweets}
          onClick={() => void store.dispatch({ type: "toggle_include_retweets" })}
        >
          拉取含转发
        </Chip>
      </div>
    </div>
  );
}

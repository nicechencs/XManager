import type { PostKind, Tweet } from "./types";

/** coalesce_impression_count: non-public wins when present. */
export function views(t: Tweet): number {
  return t.non_public_metrics.impression_count > 0
    ? t.non_public_metrics.impression_count
    : t.public_metrics.impression_count;
}

export function engagement(t: Tweet): number {
  const m = t.public_metrics;
  return (
    m.like_count + m.retweet_count + m.reply_count + m.quote_count + m.bookmark_count
  );
}

/** Derived kind (转发 > 回帖 > 引用 > 原创, with the "RT @" fallback). */
export function kindOf(t: Tweet): PostKind {
  if (t.is_retweet) return "retweet";
  if (t.in_reply_to_user_id !== null) return "reply";
  if (t.is_quote) return "quote";
  if (t.text.trimStart().startsWith("RT @")) return "retweet";
  return "original";
}

function rate(numerator: number, denominator: number): number {
  return numerator / Math.max(denominator, 1);
}

export function likeRate(t: Tweet): number {
  return rate(t.public_metrics.like_count, views(t));
}

export function bookmarkRate(t: Tweet): number {
  return rate(t.public_metrics.bookmark_count, views(t));
}

export function engagementRate(t: Tweet): number {
  return rate(engagement(t), views(t));
}

export function formatRate(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

/** `YYYY-MM-DD HH:MM` from RFC3339-ish timestamps, "-" when absent. */
export function displayDate(t: Tweet): string {
  const raw = t.created_at;
  if (!raw) return "-";
  if (raw.length >= 16) return `${raw.slice(0, 10)} ${raw.slice(11, 16)}`;
  return raw;
}

/** Compact month-day for dense tables (`MM-DD`). */
export function displayDateShort(t: Tweet): string {
  const raw = t.created_at;
  if (!raw) return "-";
  return raw.length >= 10 ? raw.slice(5, 10) : raw;
}

export const KIND_LABEL_ZH: Record<PostKind, string> = {
  original: "原创",
  reply: "回帖",
  retweet: "转发",
  quote: "引用",
};

export const KIND_SHORT_ZH: Record<PostKind, string> = {
  original: "原",
  reply: "回",
  retweet: "转",
  quote: "引",
};

/** Truncate to `max` characters (by code point) with an ellipsis. */
export function truncateText(text: string, max: number): string {
  const flat = text.replace(/\n/g, " ");
  const chars = Array.from(flat);
  if (chars.length > max) return `${chars.slice(0, max).join("")}…`;
  return flat;
}

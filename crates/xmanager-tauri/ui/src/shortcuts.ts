import type { Action, UiSnapshot } from "./types";

/**
 * Global workspace shortcuts, mirroring the GPUI key map. While the
 * delete-confirm panel is open only Escape is claimed.
 * j/↓ 下一行 · k/↑ 上一行 · space 勾选 · / 筛选 · 1–3 切页 · Esc 关闭
 *
 * The filter drawer lives in frontend state, so `/` and part of the Escape
 * priority chain resolve to the `open_drawer` / `close_drawer` markers that
 * App.tsx turns into state updates (plus the matching backend actions).
 */
export type ShortcutResolution =
  | { kind: "action"; action: Action }
  | { kind: "open_drawer" }
  | { kind: "close_drawer" }
  | { kind: "unfocus" }
  | null;

export function resolveShortcut(
  key: string,
  snapshot: UiSnapshot
): ShortcutResolution {
  const keyLower = key.toLowerCase();
  if (snapshot.cleanup.delete_confirm !== null) {
    return keyLower === "escape"
      ? { kind: "action", action: { type: "cancel_delete_confirm" } }
      : null;
  }
  switch (keyLower) {
    case "escape":
      if (snapshot.error_msg !== null) {
        return { kind: "action", action: { type: "clear_error" } };
      }
      return { kind: "close_drawer" };
    case "j":
    case "arrowdown":
      return { kind: "action", action: { type: "focus_next" } };
    case "k":
    case "arrowup":
      return { kind: "action", action: { type: "focus_prev" } };
    case " ":
      return { kind: "action", action: { type: "toggle_focused_selection" } };
    case "/":
      return { kind: "open_drawer" };
    case "1":
      return { kind: "action", action: { type: "set_route", route: "library" } };
    case "2":
      return { kind: "action", action: { type: "set_route", route: "insights" } };
    case "3":
      return { kind: "action", action: { type: "set_route", route: "cleanup" } };
    default:
      return null;
  }
}

export function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT" ||
    target.isContentEditable
  );
}

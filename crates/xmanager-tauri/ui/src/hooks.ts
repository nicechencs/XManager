import { useSyncExternalStore } from "react";
import {
  getDrawerOpen,
  getPending,
  getSnapshot,
  store,
  subscribe,
} from "./store";
import type { UiSnapshot } from "./types";

export type LayoutMode = "wide" | "medium" | "narrow";

export function layoutFromWidth(width: number): LayoutMode {
  if (width >= 1200) return "wide";
  if (width >= 800) return "medium";
  return "narrow";
}

/** Reactive access to the backend snapshot plus long-command pending label. */
export function useAppState(): {
  snapshot: UiSnapshot | null;
  pending: string | null;
} {
  const snap = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const pending = useSyncExternalStore(subscribe, getPending, getPending);
  return { snapshot: snap, pending };
}

export function useDrawerOpen(): boolean {
  return useSyncExternalStore(subscribe, getDrawerOpen, getDrawerOpen);
}

export { store };

export const LAYOUT_BREAKPOINTS = { wide: 1200, medium: 800 } as const;

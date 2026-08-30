import type { Action, ExportFormat, UiSnapshot } from "./types";
import {
  artifactName,
  confirmAndDelete,
  deletePreviewed,
  dispatchAction,
  exportCandidates,
  exportTweets,
  fetchTweets,
  initialize,
  joinPath,
  refreshWhoami,
} from "./api";

type Listener = () => void;

let snapshot: UiSnapshot | null = null;
/** Long-running backend command in flight, shown as 处理中 in the UI. */
let pending: string | null = null;
/** Filter drawer visibility — pure view state, mirrors GPUI filter_drawer_open. */
let drawerOpen = false;
let pendingGeneration = 0;
const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener();
}

function setPending(value: string | null) {
  pending = value;
  if (value !== null) pendingGeneration += 1;
  emit();
}

export function getSnapshot(): UiSnapshot | null {
  return snapshot;
}

export function getPending(): string | null {
  return pending;
}

export function getDrawerOpen(): boolean {
  return drawerOpen;
}

export function setDrawerOpen(open: boolean) {
  drawerOpen = open;
  emit();
}

export function toggleDrawer() {
  drawerOpen = !drawerOpen;
  emit();
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function apply(next: UiSnapshot) {
  snapshot = next;
  emit();
}

/** Fire a long backend command with a pending label; errors surface as toasts
 * of last resort via console + error banner is backend-driven. */
async function run(label: string, work: Promise<UiSnapshot>): Promise<void> {
  setPending(label);
  try {
    apply(await work);
  } catch (err) {
    console.error(`${label} failed`, err);
  } finally {
    setPending(null);
  }
}

export const store = {
  initialize(): Promise<void> {
    return run("启动", initialize());
  },

  openDrawer(): void {
    setDrawerOpen(true);
  },

  closeDrawer(): void {
    setDrawerOpen(false);
  },

  toggleFilterDrawer(): void {
    toggleDrawer();
  },

  dispatch(action: Action): Promise<void> {
    return run("处理", dispatchAction(action));
  },

  fetchTweets(): Promise<void> {
    return run("拉取并分析", fetchTweets());
  },

  refreshWhoami(): Promise<void> {
    return run("刷新状态", refreshWhoami());
  },

  deletePreviewed(): Promise<void> {
    return run("删除", deletePreviewed());
  },

  confirmAndDelete(): Promise<void> {
    return run("删除", confirmAndDelete());
  },

  export(scope: "library" | "cleanup", format: ExportFormat): Promise<void> {
    const kind = scope === "library" ? "library" : "cleanup";
    const name = artifactName(kind, format);
    const defaultDir = snapshot?.default_export_dir ?? "";
    return run("导出", (async () => {
      let path: string | null = null;
      let dialogFailed = false;
      try {
        const { save } = await import("@tauri-apps/plugin-dialog");
        path = await save({
          defaultPath: joinPath(defaultDir, name),
          filters: [
            format === "csv"
              ? { name: "CSV", extensions: ["csv"] }
              : { name: "JSON", extensions: ["json"] },
          ],
        });
      } catch (err) {
        console.error("save dialog failed", err);
        dialogFailed = true;
      }
      if (path === null && !dialogFailed) {
        // User cancelled the native save dialog: no write, status only.
        return dispatchAction({
          type: "set_status",
          message: scope === "library" ? "已取消导出" : "已取消备份",
        });
      }
      // dialogFailed → null path: backend falls back to the default export dir.
      return scope === "library"
        ? exportTweets(format, path)
        : exportCandidates(format, path);
    })());
  },
};

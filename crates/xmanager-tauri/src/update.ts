import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdatePhase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; body: string | null }
  | { kind: "downloading"; version: string; received: number; total: number | null }
  | { kind: "readyToRestart"; version: string }
  | { kind: "upToDate" }
  | { kind: "failed"; message: string };

let phase: UpdatePhase = { kind: "idle" };
/** Set once the user dismisses an available-update banner for this session. */
let dismissed = false;
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) listener();
}

function setPhase(next: UpdatePhase) {
  phase = next;
  emit();
}

export function getUpdatePhase(): UpdatePhase {
  return phase;
}

export function isDismissed(): boolean {
  return dismissed;
}

export function dismissUpdate() {
  dismissed = true;
  emit();
}

export function subscribeUpdate(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Query the update endpoint. `manual` controls how failures are reported:
 * silent startup checks only log; the sidebar button surfaces copy. */
export async function checkForUpdate(
  manual: boolean,
  onStatus?: (message: string) => void
): Promise<void> {
  if (phase.kind === "checking" || phase.kind === "downloading") return;
  setPhase({ kind: "checking" });
  try {
    const update: Update | null = await check();
    if (update === null) {
      setPhase({ kind: "upToDate" });
      if (manual) onStatus?.("已是最新版本");
      return;
    }
    dismissed = false;
    setPhase({
      kind: "available",
      version: update.version,
      body: update.body ?? null,
    });
  } catch (err) {
    setPhase({ kind: "idle" });
    // No manifest (first release), offline, or endpoint hiccups: expected
    // for silent checks; manual checks surface a short reason.
    const message = err instanceof Error ? err.message : String(err);
    console.warn("update check failed", err);
    if (manual) onStatus?.(`检查更新失败: ${message}`);
  }
}

/** Download + install the pending update, then ask the user to relaunch. */
export async function downloadAndInstall(): Promise<void> {
  if (phase.kind !== "available") return;
  const { version } = phase;
  setPhase({ kind: "downloading", version, received: 0, total: null });
  try {
    const update: Update | null = await check();
    if (update === null) {
      setPhase({ kind: "upToDate" });
      return;
    }
    let received = 0;
    let total: number | null = null;
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? null;
          break;
        case "Progress":
          received += event.data.chunkLength;
          setPhase({ kind: "downloading", version, received, total });
          break;
        case "Finished":
          break;
      }
    });
    setPhase({ kind: "readyToRestart", version });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error("update install failed", err);
    setPhase({ kind: "failed", message });
  }
}

export async function relaunchApp(): Promise<void> {
  await relaunch();
}

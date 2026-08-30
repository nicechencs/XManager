import { invoke } from "@tauri-apps/api/core";
import type { Action, ExportFormat, UiSnapshot } from "./types";

/** Every command answers with the fresh snapshot; the store just replaces it. */
async function runCommand(
  cmd: string,
  args?: Record<string, unknown>
): Promise<UiSnapshot> {
  return invoke<UiSnapshot>(cmd, args);
}

export function initialize(): Promise<UiSnapshot> {
  return runCommand("initialize");
}

export function dispatchAction(action: Action): Promise<UiSnapshot> {
  return runCommand("dispatch", { action });
}

export function fetchTweets(): Promise<UiSnapshot> {
  return runCommand("fetch_tweets");
}

export function refreshWhoami(): Promise<UiSnapshot> {
  return runCommand("refresh_whoami");
}

export function deletePreviewed(): Promise<UiSnapshot> {
  return runCommand("delete_previewed");
}

export function confirmAndDelete(): Promise<UiSnapshot> {
  return runCommand("confirm_and_delete");
}

export function exportTweets(
  format: ExportFormat,
  path: string | null
): Promise<UiSnapshot> {
  return runCommand("export_tweets", { format, path });
}

export function exportCandidates(
  format: ExportFormat,
  path: string | null
): Promise<UiSnapshot> {
  return runCommand("export_candidates", { format, path });
}

/** xmanager-library-YYYYMMDD-HHMMSS.ext, matching core artifact_file_stem. */
export function artifactName(kind: string, format: ExportFormat): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp =
    `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}` +
    `-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
  return `xmanager-${kind}-${stamp}.${format}`;
}

export function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;
}

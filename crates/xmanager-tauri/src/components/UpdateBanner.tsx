import { useSyncExternalStore } from "react";
import {
  dismissUpdate,
  downloadAndInstall,
  getUpdatePhase,
  isDismissed,
  relaunchApp,
  subscribeUpdate,
} from "../update";
import { Btn } from "./common";

function formatBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(n / 1024))} KB`;
}

/** In-app updater banner: available → downloading → restart prompt. */
export function UpdateBanner() {
  const phase = useSyncExternalStore(subscribeUpdate, getUpdatePhase, getUpdatePhase);
  const dismissed = useSyncExternalStore(subscribeUpdate, isDismissed, isDismissed);

  if (phase.kind === "available" && !dismissed) {
    return (
      <div className="banner accent">
        <span className="banner-text">
          <span className="outcome-headline">发现新版本 v{phase.version}</span>
          {phase.body ? <span className="caption"> {phase.body}</span> : null}
        </span>
        <span className="banner-actions">
          <Btn variant="primary" onClick={() => void downloadAndInstall()}>
            下载并安装
          </Btn>
          <Btn variant="ghost" onClick={dismissUpdate}>
            忽略
          </Btn>
        </span>
      </div>
    );
  }

  if (phase.kind === "downloading") {
    const pct =
      phase.total !== null && phase.total > 0
        ? Math.round((phase.received / phase.total) * 100)
        : null;
    return (
      <div className="banner accent">
        <span className="banner-text">
          正在下载 v{phase.version}… {formatBytes(phase.received)}
          {phase.total !== null ? ` / ${formatBytes(phase.total)}` : ""}
          {pct !== null ? `（${pct}%）` : ""}
        </span>
      </div>
    );
  }

  if (phase.kind === "readyToRestart") {
    return (
      <div className="banner accent">
        <span className="banner-text">
          <span className="outcome-headline">
            v{phase.version} 已就绪，重启应用完成更新
          </span>
        </span>
        <span className="banner-actions">
          <Btn variant="primary" onClick={() => void relaunchApp()}>
            立即重启
          </Btn>
        </span>
      </div>
    );
  }

  if (phase.kind === "failed") {
    return (
      <div className="banner warn">
        <span className="banner-text">更新下载失败：{phase.message}</span>
        <span className="banner-actions">
          <Btn variant="ghost" onClick={() => dismissUpdate()}>
            关闭
          </Btn>
        </span>
      </div>
    );
  }

  return null;
}

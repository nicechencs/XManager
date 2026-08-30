# XManager 日志规范

本文是本地日志的设计源：覆盖哪些操作、文件如何命名、保留多久、哪些字段禁止出现。
实现位于 `xmanager-core::logging`，桌面端在启动时 `init_best_effort()`。

## 目标

1. **可审计**：备份 / 预演 / 真删等不可逆流程必须落盘，且不因 `LOG_LEVEL=error` 被丢掉。
2. **可诊断**：API 失败、429 限流、导出失败、配置缺失要能事后对照。
3. **不泄密**：OAuth 密钥、Authorization、推文正文永不写入日志。
4. **可清理**：按文件名中的日期自动过期，不依赖 mtime。

日志是给排障和安全追溯用的；界面上的 `status_msg` / `error_msg` 仍是瞬时 UX，进程退出即消失。

## 双流与保留

| 流 (`stream`) | 用途 | 默认保留 | 级别策略 |
|---------------|------|----------|----------|
| `app` | 启动、配置、API、导出、UI 错误 | **14 天** | 受 `XMANAGER_LOG_LEVEL` 过滤 |
| `audit` | 备份、预演、删除、取消删除 | **90 天** | **Info 及以上始终写入**（不受 min_level 影响） |

保留规则：文件名日期 `< today - retention_days` 的文件在启动时删除。
例：今天 `2026-08-15`、app 保留 14 天 → `xmanager-app-2026-08-01.log` 保留，`2026-07-31` 删除。

`audit` 在 Info+ 写入后 `flush` + `sync_data`，降低真删记录因崩溃丢失的概率。

## 文件命名

目录默认是数据根下的 `logs/`：

- 仓库内 `cargo run`：当前目录的 `./logs`（与 `./exports` 一样，已被 `.gitignore`）
- 打包后的桌面程序：用户配置目录，Windows 为 `%APPDATA%\XManager\logs`，macOS 为 `~/Library/Application Support/XManager/logs`
- 可用 `XMANAGER_LOG_DIR` 覆盖日志目录，或用 `XMANAGER_DATA_DIR` 改整个数据根

```
logs/xmanager-{stream}-{YYYY-MM-DD}.log
```

| 示例 | 含义 |
|------|------|
| `xmanager-app-2026-08-15.log` | 当天诊断日志 |
| `xmanager-audit-2026-08-15.log` | 当天安全审计 |

规则：

- 前缀固定 `xmanager-`
- 流名只有 `app` / `audit`
- 日期必须是 `YYYY-MM-DD`（ISO，连字符）
- 扩展名 `.log`（JSON Lines，一行一条）
- 不识别的文件名（如 `notes.txt`）**不会**被保留清理删除

导出 / 备份文件（不是日志，但同一套命名）：

```
xmanager-library-YYYYMMDD-HHMMSS.{csv|json}
xmanager-cleanup-YYYYMMDD-HHMMSS.{csv|json}
```

不再使用含糊的 `low_*.csv` 或混用下划线的 `xmanager_cleanup_*`。

## 记录格式（JSONL）

```json
{
  "ts": "2026-08-15T23:20:01.123+08:00",
  "level": "info",
  "stream": "audit",
  "event": "cleanup.delete",
  "outcome": "partial",
  "fields": {
    "revision": 4,
    "requested": 3,
    "succeeded": 2,
    "failed": 1
  }
}
```

| 字段 | 规则 |
|------|------|
| `ts` | RFC3339，毫秒，带本地偏移 |
| `level` | `debug` / `info` / `warn` / `error` |
| `stream` | `app` / `audit` |
| `event` | `{area}.{action}`，小写，见事件目录 |
| `outcome` | 可选：`ok` / `error` / `cancel` / `partial` |
| `fields` | 蛇形小写；只放标量 / id 列表，不放正文 |

事件名校验：至少两段，每段 `[a-z][a-z0-9_]*`。合法：`cleanup.delete`。非法：`fetch`、`API.request`。

## 事件目录

| event | stream | 默认级别 | 触发点 | 典型 fields |
|-------|--------|----------|--------|-------------|
| `app.start` | app | info | 进程启动 | `version`, `file_logging` |
| `app.config` | app | info/warn/error | 读取 `.env` | `oauth1`, `has_api_key`…（仅有无，无值）, `env_file`（路径，可选） |
| `api.request` | app | debug / error | 每次 HTTP | `http_method`, `url`（去 query）, `http_status`, `duration_ms` |
| `api.rate_limited` | app | warn | HTTP 429 并准备等待 | `sleep_secs`, `retry` |
| `account.whoami` | app | info/error | `GET /users/me` | `user_id`, `username`, `duration_ms` |
| `timeline.fetch` | app | info/error | 分页拉完或失败 | `user_id`, `limit`, `count`, `pages` |
| `tweet.lookup` | app | info | `GET /2/tweets?ids=` 预演 | `requested`, `found`, `missing`, `failed` |
| `export.write` | app | info | CSV/JSON 写成功 | `format`, `count`, `path` |
| `tweet.delete` | **audit** | info/error | 单条 `DELETE /tweets/:id` | `tweet_id`, `deleted` |
| `cleanup.backup` | **audit** | info/error | 候选备份 | `revision`, `count`, `path`, `receipt_bound` |
| `cleanup.preview` | **audit** | info | 预演分类完成 | `revision`, `deletable`, `missing`, `candidate_ids` |
| `cleanup.delete` | **audit** | info/error | 批量真删汇总 | `requested`, `succeeded_ids`, `failed_ids` |
| `cleanup.cancel` | **audit** | info | 用户取消确认/对话框 | `stage` / `dry_run`, `revision` |
| `ui.error` | app | error | `AppState::set_error` | `message`（已是用户可见摘要） |

不记：勾选切换、主题、路由、焦点移动（噪声大、无审计价值）。

## 禁止写入

- `X_API_KEY` / `X_API_SECRET` / `X_ACCESS_TOKEN` / `X_ACCESS_TOKEN_SECRET` / `X_BEARER_TOKEN` 的值
- `Authorization` 头
- 推文 `text`
- URL query（含 `pagination_token`）

`Settings` 的 `Debug` 只输出 `<redacted>` / `<empty>`。
字段名若匹配敏感子串（`token`、`secret`、`authorization`…）且值为字符串，写出前会被替换为 `[redacted]`；布尔“是否已配置”可以保留。

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `XMANAGER_LOG_DIR` | 数据根下的 `logs/` | 日志目录 |
| `XMANAGER_DATA_DIR` | 见上 | 数据根（其下 `logs/` 与 `exports/`） |
| `XMANAGER_LOG_LEVEL` | `info` | `debug` \| `info` \| `warn` \| `error` |
| `XMANAGER_LOG_APP_RETENTION_DAYS` | `14` | app 流保留天数 |
| `XMANAGER_LOG_AUDIT_RETENTION_DAYS` | `90` | audit 流保留天数 |

目录无法创建时：向 stderr 打印原因，应用继续运行；Warn/Error 回退到 stderr（JSONL 一行）。

排障时可临时：

```bash
XMANAGER_LOG_LEVEL=debug cargo run -p xmanager-tauri --release --features custom-protocol
```

## 覆盖对照（实现前 → 现在）

| 操作 | 实现前 | 现在 |
|------|--------|------|
| 启动 / 读配置 | 无 | `app.start` / `app.config` |
| whoami / 拉时间线 | 仅内存 status | `account.whoami` / `timeline.fetch` + 分页 debug |
| 429 限流休眠 | 静默 `thread::sleep` | `api.rate_limited` |
| HTTP 失败 | 仅 UI 红字 | `api.request` error |
| 导出 / 备份 | 仅 status | `export.write` + `cleanup.backup`（audit） |
| 预演 / 真删 / 取消 | 仅内存 receipt | `cleanup.preview` / `cleanup.delete` / `cleanup.cancel` + 逐条 `tweet.delete` |
| 密钥 `Debug` | `#[derive(Debug)]` 会打印原文 | 已脱敏 |

## 库用法

```rust
use xmanager_core::logging::{self, events, LogConfig, Outcome, Stream};

logging::init(LogConfig::from_env())?;
logging::info(Stream::App, events::TIMELINE_FETCH)
    .outcome(Outcome::Ok)
    .field("count", 87_u64)
    .emit();
```

测试请构造独立的 `Logger::open(LogConfig { dir: tmp, .. })`，不要依赖进程全局 sink。

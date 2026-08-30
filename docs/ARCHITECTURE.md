# XManager 架构与目录规划

## 仓库布局

```
XManager/
├── Cargo.toml                 # Rust workspace 根
├── .env.example               # API 凭证模板（勿提交 .env）
├── run.sh / run.bat           # macOS·Linux / Windows 启动
├── packaging/                 # 发版 zip 内 README、macOS Info.plist
├── scripts/                   # Windows zip / macOS .app
├── .github/workflows/         # PR CI 与 tag Release
├── README.md                  # 主文档
├── docs/
│   ├── ARCHITECTURE.md        # 本文件
│   ├── DESIGN.md              # 字号 / 间距 / 交互约定
│   ├── LOGGING.md
│   └── RELEASE.md             # tag 与 GitHub Actions 发版
├── crates/
│   ├── xmanager-core/         # 领域库：API / 筛选 / 导出（无 UI）
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs      # X API v2 + OAuth1
│   │       ├── config.rs      # Settings / .env
│   │       ├── error.rs
│   │       ├── export.rs      # CSV / JSON
│   │       ├── filter.rs      # 筛选 + 统计 + 比率/时段
│   │       ├── logging.rs     # JSONL 双流日志（app 14d / audit 90d）
│   │       ├── models.rs      # Tweet / User / PostKind / metrics
│   │       └── paths.rs       # .env / logs / exports 目录
│   ├── xmanager-cli/          # 命令行（JSON stdout，供自动化测试）
│   └── xmanager-tauri/        # 桌面端 Tauri 应用（二进制 xmanager）
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── src-tauri/         # Tauri 壳（Cargo 包 xmanager-tauri）
│       │   ├── tauri.conf.json
│       │   └── src/           # Rust 命令层与状态（权威状态在此）
│       │       ├── state.rs   # 工作台状态机（筛选/选择/安全清理工作流）
│       │       ├── commands.rs# Tauri 命令（每个命令返回完整快照）
│       │       ├── actions.rs # 前端动作枚举
│       │       └── dto.rs     # 可序列化视图快照
│       └── src/…  index.html  package.json   # React + TypeScript + Vite 前端（crate 根，纯视图层）
└── exports/                   # 开发时导出目录（gitignore；打包后见用户配置目录）
└── logs/                      # 开发时日志（gitignore；见 docs/LOGGING.md）
```

## 设计原则

1. **UI 与领域分离**：所有 X API、筛选、导出逻辑在 `xmanager-core`；Tauri 命令层只做编排；React 前端是快照的纯视图。
2. **阻塞 IO 在后台线程**：`reqwest::blocking` 通过 `spawn_blocking` 在异步运行时的阻塞池执行，完成后回写状态并返回新快照。
3. **凭证只来自环境**：`.env` / 环境变量；永不写死密钥。`.env` 查找顺序：可执行文件旁边 → `XMANAGER_DATA_DIR` → 当前目录向上两级 → 用户配置目录。打包后的日志/导出写到用户配置目录（或 exe 旁已有 `.env` 的便携目录）。
4. **删除需确认**：内容库可直接「删除选中」（一次确认 + 自动备份）；安全清理仍绑定 revision/receipt，真删前自动备份 + 预演，再确认一次。
5. **目录干净**：根目录只放 workspace 配置与文档；实现代码只在 `crates/*`。
6. **本地可审计日志**：诊断写 `logs/xmanager-app-YYYY-MM-DD.log`（14 天）；备份/预演/删除写 `logs/xmanager-audit-YYYY-MM-DD.log`（90 天，Info+ 不丢）。密钥与推文正文禁止入日志。详见 [LOGGING.md](LOGGING.md)。

## UI 壳层

- **路由**：`Library | Insights | Cleanup`（共享后端 `Workspace`，切页不丢筛选/候选）。
- **筛选**：抽屉改条件后立刻写入 `applied_filter` 并重算列表。拉取条数 / 含转发与筛选分开，只影响下次 API 请求。chip 只显示非默认条件，可逐个移除。
- **安全清理会话**：`cleanup_candidates` + `cleanup_snapshot` + `cleanup_revision` + backup/preview receipts。加入候选不切页；`cleanup_notice` 横幅提供「去安全清理」。内容库「删除选中」走独立确认，不要求先加入候选。
- **视觉**：色板、字号、行高、间距 token 在前端 `ui/src/theme.css` 的 CSS 变量（移植自原 GPUI `theme.rs` 调色板），约定见 [DESIGN.md](DESIGN.md)。
- **响应式**：`wide ≥1200px / medium ≥800px / narrow` 由前端监听视口宽度切换。宽屏检查器仅在选中推文时占列；窄屏筛选/检查器为全高覆盖层（不同时并排），列表为卡片行。
- **列表**：`@tanstack/react-virtual` 虚拟化渲染筛选结果。正文优先（两行），日期与曝光为次要列。
- **洞察**：全部已同步数据的汇总 + 曝光直方图（当前切片高亮）；点柱或预设回内容库。
- **状态架构**：每个 Tauri 命令返回完整 `UiSnapshot`（推文、筛选、统计、清理工作流、状态行）。前端不持有业务状态，只渲染快照并把交互映射为命令调用。

## 数据流

```
.env → Settings → XClient
                    ├─ get_me()
                    ├─ fetch_own_tweets(limit, exclude_rt, exclude_replies)
                    ├─ lookup_tweets(ids)   # 预演：可删 / 缺失 / 失败
                    └─ delete_tweet(id)     # 429 立即停，不再连删连睡

tweets[] → FilterOptions
            (time_range / kinds / rates / top_n / max_views …)
         → filter_tweets → 内容库列表 / 数据洞察 / 导出
         → 内容库「删除选中」→ confirm → backup → delete
         → cleanup candidates → backup → preview → confirm → delete
```

## 功能对照

| 功能 | core | UI |
|------|------|----|
| 认证 / whoami | `get_me` | 侧栏凭证状态 |
| 拉取时间线 | `fetch_own_tweets` | 内容库拉取 + 最后同步时间 |
| 时间段 / 类型 / 比率排序 | `TimeRange` `KindFilter` `SortField` | 筛选抽屉 + 可移除 chip |
| 低曝光筛选 | `FilterOptions` | ≤10/20/50/100 快捷 |
| 统计 | `summarize` / histogram | 洞察看 `all_tweets`；当前筛选为切片高亮 |
| 排名 | 低曝光样本 | 洞察 5 条样本，点击回内容库并聚焦 |
| 导出 | `export_csv` / `export_json` | 内容库 / 洞察 / 清理备份 |
| 安全删除 | `lookup_tweets` + `delete_tweet(s)` | 加入后留在内容库；「删除 N 条」自动备份+预演 → 确认一次 → 真删 |
| 本地日志 | `logging` | 启动初始化；API / 导出 / 清理审计（见 [LOGGING.md](LOGGING.md)） |
| 外观 | — | 侧栏浅色/深色 |
| 响应式 | — | Wide 三栏；Medium 紧凑导航；Narrow 覆盖层 + 卡片 |
| 命令行自动化 | `xmanager-cli` | — |

`xmanager-cli` 与 UI 共用 `xmanager-core`。Windows 上桌面二进制没有控制台，所以 CLI 是独立 crate。CLI 无 GUI 依赖，可在无显示器的 macOS / Linux 上使用。

## 构建

桌面端（`xmanager-tauri`）基于 Tauri 2：Windows 用 WebView2，macOS 用 WKWebView，Linux 用 WebKitGTK。系统包见根目录 [README.md](../README.md)。

```bash
# 领域库测试
cargo test -p xmanager-core

# 命令行（离线测试；无显示器也可）
cargo test -p xmanager-cli
cargo run -p xmanager-cli -- --help
# 或 ./run.sh cli -- --help   /   run.bat cli -- --help

# 桌面应用（先构建前端）
(cd crates/xmanager-tauri && npm install && npm run build)
cargo run -p xmanager-tauri --release --features custom-protocol
# 或 ./run.sh   /   run.bat
# 前端热更新开发：cd crates/xmanager-tauri && npm run tauri dev
```

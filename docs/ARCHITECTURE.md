# XManager 架构与目录规划

## 仓库布局

```
XManager/
├── Cargo.toml                 # Rust workspace 根
├── .env.example               # API 凭证模板（勿提交 .env）
├── run.sh / run.bat           # macOS·Linux / Windows 启动
├── README.md                  # 主文档
├── docs/
│   ├── ARCHITECTURE.md        # 本文件
│   ├── DESIGN.md              # 字号 / 间距 / 交互约定
│   └── LOGGING.md
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
│   │       └── models.rs      # Tweet / User / PostKind / metrics
│   ├── xmanager-cli/          # 命令行（JSON stdout，供自动化测试）
│   └── xmanager-ui/           # 桌面端 GPUI 应用
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── theme.rs
│           ├── widgets.rs
│           └── views/
└── exports/                   # 运行时导出目录（gitignore）
└── logs/                      # 运行时日志（gitignore；见 docs/LOGGING.md）
```

## 设计原则

1. **UI 与领域分离**：所有 X API、筛选、导出逻辑在 `xmanager-core`；GPUI 只负责展示与交互。
2. **阻塞 IO 在后台线程**：`reqwest::blocking` 不在 UI 帧内调用；通过 `background_executor` / 后台任务回写状态。
3. **凭证只来自环境**：`.env` / 环境变量；永不写死密钥。
4. **删除需确认**：安全清理绑定 revision/receipt；真删前自动备份 + 预演，再确认一次。
5. **目录干净**：根目录只放 workspace 配置与文档；实现代码只在 `crates/*`。
6. **本地可审计日志**：诊断写 `logs/xmanager-app-YYYY-MM-DD.log`（14 天）；备份/预演/删除写 `logs/xmanager-audit-YYYY-MM-DD.log`（90 天，Info+ 不丢）。密钥与推文正文禁止入日志。详见 [LOGGING.md](LOGGING.md)。

## UI 壳层

- **路由**：`Library | Insights | Cleanup`（共享 `AppState`，切页不丢筛选/候选）。
- **筛选**：抽屉改条件后立刻写入 `applied_filter` 并重算列表。拉取条数 / 含转发与筛选分开，只影响下次 API 请求。chip 只显示非默认条件，可逐个移除。
- **安全清理会话**：`cleanup_candidates` + `cleanup_snapshot` + `cleanup_revision` + backup/preview receipts。加入候选不切页；`cleanup_notice` 横幅提供「去安全清理」。
- **视觉**：色板、字号、行高、间距 token 在 `xmanager-ui` 的 `theme.rs`，约定见 [DESIGN.md](DESIGN.md)。
- **响应式**：`LayoutMode::{Wide,Medium,Narrow}` 由窗口宽度每帧同步。窄屏筛选/检查器为全高覆盖层（不同时并排），列表为卡片行。
- **列表**：`uniform_list` 虚拟化渲染筛选结果。
- **洞察**：KPI + 直方图 + 当前筛选 Top-N 排名（点击回内容库并聚焦）。

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
         → cleanup candidates → backup → preview → confirm → delete
```

## 功能对照

| 功能 | core | UI |
|------|------|----|
| 认证 / whoami | `get_me` | 侧栏凭证状态 |
| 拉取时间线 | `fetch_own_tweets` | 内容库拉取 + 最后同步时间 |
| 时间段 / 类型 / 比率排序 | `TimeRange` `KindFilter` `SortField` | 筛选抽屉 + 可移除 chip |
| 低曝光筛选 | `FilterOptions` | ≤10/20/50/100 快捷 |
| 统计 | `summarize` / histogram | 数据洞察；直方图可点筛选 |
| 排名 | 当前 `filtered` 顺序 | 洞察 Top-N 列表，点击回内容库 |
| 导出 | `export_csv` / `export_json` | 内容库 / 洞察 / 清理备份 |
| 安全删除 | `lookup_tweets` + `delete_tweet(s)` | 加入后留在内容库；「删除 N 条」自动备份+预演 → 确认一次 → 真删 |
| 本地日志 | `logging` | 启动初始化；API / 导出 / 清理审计（见 [LOGGING.md](LOGGING.md)） |
| 外观 | — | 侧栏浅色/深色 |
| 响应式 | — | Wide 三栏；Medium 紧凑导航；Narrow 覆盖层 + 卡片 |
| 命令行自动化 | `xmanager-cli` | — |

`xmanager-cli` 与 UI 共用 `xmanager-core`。Windows 上 GPUI 二进制没有控制台，所以 CLI 是独立 crate。CLI 无 GUI 依赖，可在无显示器的 macOS / Linux 上使用。

## 构建

桌面端（`xmanager-ui`）依赖 GPUI 0.2.2：macOS 用 Metal，Linux 用 Wayland 或 X11 + Vulkan，Windows 用现有后端。系统包见根目录 [README.md](../README.md)。

```bash
# 领域库测试
cargo test -p xmanager-core

# 命令行（离线测试；无显示器也可）
cargo test -p xmanager-cli
cargo run -p xmanager-cli -- --help
# 或 ./run.sh cli -- --help   /   run.bat cli -- --help

# 桌面应用
cargo run -p xmanager-ui --release
# 或
cargo run --release
# 或 ./run.sh   /   run.bat
```

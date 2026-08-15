# XManager 架构与目录规划

## 仓库布局

```
XManager/
├── Cargo.toml                 # Rust workspace 根
├── .env.example               # API 凭证模板（勿提交 .env）
├── README.md                  # 主文档
├── docs/
│   └── ARCHITECTURE.md        # 本文件
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
│   │       └── models.rs      # Tweet / User / PostKind / metrics
│   └── xmanager-ui/           # 桌面端 GPUI 应用
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── theme.rs
│           ├── widgets.rs
│           └── views/
└── exports/                   # 运行时导出目录（gitignore）
```

## 设计原则

1. **UI 与领域分离**：所有 X API、筛选、导出逻辑在 `xmanager-core`；GPUI 只负责展示与交互。
2. **阻塞 IO 在后台线程**：`reqwest::blocking` 不在 UI 帧内调用；通过 `background_executor` / 后台任务回写状态。
3. **凭证只来自环境**：`.env` / 环境变量；永不写死密钥。
4. **删除需确认**：安全清理绑定 revision/receipt；真删前备份 + 预演 + 二次确认（数量/`DELETE`）。
5. **目录干净**：根目录只放 workspace 配置与文档；实现代码只在 `crates/*`。

## UI 壳层

- **路由**：`Library | Insights | Cleanup`（共享 `AppState`，切页不丢筛选/候选）。
- **筛选**：抽屉草稿 `filter_draft` vs 已生效 `applied_filter`；chip 可逐个移除。
- **安全清理会话**：`cleanup_candidates` + `cleanup_snapshot` + `cleanup_revision` + backup/preview receipts。
- **响应式**：`LayoutMode::{Wide,Medium,Narrow}` 由窗口宽度每帧同步；窄屏默认无检查器，聚焦推文时打开覆盖列。
- **列表**：`uniform_list` 虚拟化渲染筛选结果。

## 数据流

```
.env → Settings → XClient
                    ├─ get_me()
                    ├─ fetch_own_tweets(limit, exclude_rt, exclude_replies)
                    └─ delete_tweet(id)

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
| 导出 | `export_csv` / `export_json` | 内容库 / 洞察 / 清理备份 |
| 安全删除 | `delete_tweet(s)` | 候选 → 备份 → 预演 → 二次确认 → 真删 |
| 响应式 | — | `LayoutMode` Wide/Medium/Narrow |

## 构建

```bash
# 领域库测试
cargo test -p xmanager-core

# 桌面应用
cargo run -p xmanager-ui --release
# 或
cargo run --release
```

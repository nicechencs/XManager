# XManager

快速筛选并管理你自己**访问量较低**的 X（推特）推文。

主程序为 **Rust + GPUI** 桌面应用；领域逻辑在 `xmanager-core`。

## 功能

- 拉取自己的推文（含 `impression_count` **曝光**）
- **时间段**：24h / 7天 / 30天 / 90天 / 180天 / 360天 / 全部
- **帖子类型标记**：原创 / 回帖 / 转发 / 引用（可筛选）
- **排序指标**：曝光、点赞率、收藏率、互动率、转发率、回复率、互动量、日期
- **最高 / 最低** + Top-N（例如 7 天点赞率最高 Top20）
- 快捷：低曝光阈值（≤10/20/50/100）
- 三段工作台：**内容库** / **数据洞察** / **安全清理**
- 表格 + 勾选 + 统计（均值 / 中位 / 比率 / 类型分布 / 直方图）
- 直方图区间点击可回到内容库并应用对应曝光筛选
- 导出 CSV / JSON（内容库 / 洞察 / 清理备份）
- **安全删除**：加入候选 → 备份 → 预演（可删/缺失）→ 数量/`DELETE` 二次确认 → 真删
- 响应式布局：宽屏完整导航+检查器；中屏紧凑导航；窄屏默认隐藏检查器（点击推文打开）
- 列表虚拟化；刷新中保留旧数据并提示；空状态区分无凭证 / 无数据 / 无匹配

## 目录结构

```
XManager/
├── Cargo.toml                 # workspace
├── .env.example
├── crates/
│   ├── xmanager-core/         # API · 筛选 · 导出
│   └── xmanager-ui/           # GPUI 桌面端（二进制 xmanager）
└── docs/ARCHITECTURE.md
```

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 前置条件

1. **Rust** stable（已在 `1.89+` 验证）
2. **X Developer App**：[developer.x.com](https://developer.x.com) / [console.x.com](https://console.x.com)
3. **OAuth 1.0a 四件套**，删除需要 **Read and Write**
4. Windows / macOS / Linux（GPUI 0.2.2 含 Windows 后端）

> API 多为按量付费；读自己的时间线一般为 owned reads。时间线通常最多约最近 3200 条。

## 配置

```bash
cp .env.example .env
```

填写：

| 变量 | 说明 |
|------|------|
| `X_API_KEY` | Consumer Key |
| `X_API_SECRET` | Consumer Secret |
| `X_ACCESS_TOKEN` | User Access Token |
| `X_ACCESS_TOKEN_SECRET` | User Access Token Secret |

### Developer Portal 简要步骤

1. 创建 Project + App  
2. 开启 **Read and Write**  
3. 生成 User Access Token / Secret  
4. 将 Key/Secret/Token 写入 `.env`

## 构建与运行

Windows 可在仓库根目录双击或执行：

```bat
run.bat              :: release 编译并启动（默认）
run.bat debug        :: debug 编译并启动
run.bat bin          :: 只跑已有 release 二进制，不重新编译
run.bat debug bin    :: 只跑已有 debug 二进制
```

或直接用 Cargo：

```bash
# 在仓库根目录
cargo run -p xmanager-ui --release
# 或
cargo run --release
# 已编译时
./target/release/xmanager.exe
```

首次会编译 GPUI 及其依赖，耗时较长，属正常现象。

### 界面操作

1. 确认侧栏凭证状态为已配置  
2. 在**内容库**打开筛选：时间段、类型、排序、Top-N、低曝光快捷  
3. 点击 **拉取并分析**，再 **应用筛选**（生效条件可逐个移除）  
4. 勾选推文 → **加入安全清理**  
5. 在**安全清理**：备份 CSV/JSON → 预演 → 选择「确认数量」或「DELETE」→ 真删  

## 领域库（无 UI）

```bash
cargo test -p xmanager-core
```

```rust
use xmanager_core::{Settings, XClient, FilterOptions, filter_tweets, export_csv};

let client = XClient::new(Settings::load()?)?;
let (_me, tweets) = client.fetch_own_tweets(100, true, true)?;
let low = filter_tweets(&tweets, &FilterOptions::low_views(50));
export_csv(&low, "exports/low.csv")?;
```

## 注意

- **删除不可恢复**，务必先导出或 dry-run  
- 注意 API 速率限制与费用  
- `impression_count` 为展示次数，可能与网页统计略有差异  

## License

MIT

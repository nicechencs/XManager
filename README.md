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
- 表格 + 勾选 + 统计（均值 / 中位 / 比率 / 类型分布 / 直方图）；洞察页有 Top-N 排名，点击回到内容库
- 直方图区间点击可回到内容库并应用对应曝光筛选
- 导出 CSV / JSON（内容库 / 洞察 / 清理备份）
- 浅色 / 深色外观切换
- **安全删除**：加入候选 → 点「真实删除」（自动备份到 `exports/`，并向 X API 预演可删/缺失/失败）→ 选择确认数量或 `DELETE` → 真删。遇 HTTP 429 立即停止后续删除。
- 响应式：宽屏三栏；中屏紧凑导航；窄屏筛选/检查器为全高覆盖层，列表改为带字段标签的卡片
- 列表虚拟化；刷新中保留旧数据并提示；空状态区分无凭证 / 无数据 / 无匹配

## 目录结构

```
XManager/
├── Cargo.toml                 # workspace
├── .env.example
├── run.sh                     # macOS / Linux 启动
├── run.bat                    # Windows 启动
├── crates/
│   ├── xmanager-core/         # API · 筛选 · 导出
│   ├── xmanager-cli/          # 命令行（JSON stdout）
│   └── xmanager-ui/           # GPUI 桌面端（二进制 xmanager）
└── docs/ARCHITECTURE.md
└── docs/LOGGING.md            # 日志命名 / 保留 / 事件目录
```

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 与 [docs/LOGGING.md](docs/LOGGING.md)。

## 前置条件

1. **Rust** stable（已在 `1.89+` 验证）
2. **X Developer App**：[developer.x.com](https://developer.x.com) / [console.x.com](https://console.x.com)
3. **OAuth 1.0a 四件套**，删除需要 **Read and Write**
4. **Windows / macOS / Linux** 均可编译运行（GPUI 0.2.2：macOS 用 Metal，Linux 用 Wayland 或 X11 + Vulkan）

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
| `XMANAGER_LOG_DIR` | 可选，日志目录，默认 `logs` |
| `XMANAGER_LOG_LEVEL` | 可选，`debug` / `info` / `warn` / `error`，默认 `info` |

### Developer Portal 简要步骤

1. 创建 Project + App  
2. 开启 **Read and Write**  
3. 生成 User Access Token / Secret  
4. 将 Key/Secret/Token 写入 `.env`

## 构建与运行

本仓库是纯 Rust，**macOS、Linux、Windows 都可以执行**。

| 入口 | 适用场景 |
|------|----------|
| `xmanager-cli` | 无 GUI 依赖；无显示器的 Linux / CI / 自动化也能跑 |
| `xmanager`（桌面端） | 需要图形会话：macOS（Metal）、Linux（Wayland 或 X11 + Vulkan）、Windows |

### 系统依赖

**所有平台**

1. Rust stable（已在 `1.89+` 验证）：https://rustup.rs
2. 仓库根目录的 `.env`（见上文）

**macOS**

- 安装 [Xcode](https://developer.apple.com/xcode/) 或 Command Line Tools（桌面端用 Metal 渲染）：

```bash
xcode-select --install
```

**Linux（Debian / Ubuntu 示例）**

桌面端编译需要 clang、pkg-config，以及 Wayland/X11、Vulkan、字体相关开发包：

```bash
sudo apt install -y clang pkg-config \
  libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev \
  libfontconfig-dev libfreetype-dev \
  libvulkan-dev libvulkan1 mesa-vulkan-drivers
```

运行桌面窗口还需要：

1. 图形会话（`WAYLAND_DISPLAY` 或 `DISPLAY`）
2. 可用的 Vulkan 设备（本机 GPU 驱动；无独显的虚拟机可装 `mesa-vulkan-drivers` 走 lavapipe 软件渲染）

SSH / CI / 无显示器环境请用 CLI，不要启动桌面窗口。GPUI 在找不到 GPU 时会直接退出。

**Windows**

安装 Rust（MSVC 工具链）即可；可用 `run.bat`。

### 启动脚本

macOS / Linux：

```bash
chmod +x run.sh          # 只需一次
./run.sh                 # release 编译并启动（默认）
./run.sh debug           # debug 编译并启动
./run.sh bin             # 只跑已有 release 二进制，不重新编译
./run.sh debug bin       # 只跑已有 debug 二进制
./run.sh cli -- --help   # 命令行
```

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
# 已编译时（macOS / Linux）
./target/release/xmanager
# Windows
./target/release/xmanager.exe
```

首次会编译 GPUI 及其依赖，耗时较长，属正常现象。

## 命令行（自动化 / 测试）

桌面窗口没有控制台。自动化走独立二进制 `xmanager-cli`，**stdout 为 JSON**。

```bash
cargo run -p xmanager-cli -- --help
# 或
./run.sh cli -- --help      # macOS / Linux
run.bat cli -- --help       # Windows
```

| 命令 | 网络 | 作用 |
|------|------|------|
| `creds` | 否 | 检查四项 OAuth 是否已配置 |
| `filter` / `summarize` / `export` | 否 | 对本地推文 JSON 筛选 / 统计 / 导出 |
| `delete` | 否（默认） | 打印将删的 id；加 `--yes` 才真删 |
| `whoami` / `fetch` | 是 | 调 X API |

```bash
# 不打 API：用夹具测筛选
cargo run -p xmanager-cli -- filter --input tweets.json --max-views 20

# 凭证检查（可指定 .env）
cargo run -p xmanager-cli -- creds --env .env

# 预演删除（不会调用 DELETE）
cargo run -p xmanager-cli -- delete --ids 111,222
```

退出码：`0` 成功，`2` 缺凭证，`3` API/网络/限速（`code: rate_limited` 含 `retry_after_secs`），`1` 其它错误。

```bash
cargo test -p xmanager-cli          # 离线
cargo test -p xmanager-cli -- --ignored   # 可选：打真实 API（whoami）
```

### 界面操作

1. 确认侧栏凭证状态为已配置  
2. 在**内容库**打开筛选：时间段、类型、排序、Top-N、低曝光快捷  
3. 点击 **拉取并分析**，再 **应用筛选**（生效条件可逐个移除）  
4. 勾选左侧方框（或点开一条）→ **加入安全清理**  
5. 在**安全清理**点 **真实删除**（自动备份并问 X 预演），再**输入数量或 DELETE**（也可点 chip），然后 **确认并删除**

快捷键：`J` / `K` 或方向键上下条，空格勾选当前条，`/` 打开筛选，`Esc` 关闭抽屉/检查器/错误，`1` `2` `3` 切换内容库 / 洞察 / 清理。列表表头可点击切换排序。

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
- 诊断与删除审计写在 `logs/`（app 保留 14 天，audit 保留 90 天）；不含密钥与推文正文 

## License

MIT

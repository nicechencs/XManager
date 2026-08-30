# 发版

流程对齐 [AgentHub](https://github.com/nicechencs/AgentHub) 的 GitHub Actions 骨架：`v*` tag 触发、Windows / macOS **各自原生编译**、草稿 Release。XManager 是 GPUI 桌面程序，没有 Tauri 签名 / updater / `release` 分支。

## 产物

每个 tag 产出：

| 文件 | 内容 |
|------|------|
| `XManager-<ver>-windows-x64.zip` | `XManager.exe` + `xmanager-cli.exe` + `.env.example` + `README.txt` + `LICENSE` |
| `XManager-<ver>-macos-universal.zip` | `XManager.app`（Intel + Apple Silicon）+ `xmanager-cli` + 同上文档 |
| `SHA256SUMS.txt` | 两个 zip 的 SHA-256 |

构建是**未签名**的。Windows 可能出 SmartScreen；macOS 需要 `xattr -cr XManager.app` 或右键打开。zip 内 `README.txt` 有说明。

## 打 tag

1. 把 `[workspace.package] version` 改成要发的版本（例如 `0.1.0`）。
2. 提交并推到 `dev`（或合进 `main`）。**tag 指向的提交必须已经在 `origin/dev` 或 `origin/main` 上**。
3. 打与 Cargo 版本一致的 tag（前缀 `v`）：

```bash
git tag -a v0.1.0 -m "XManager 0.1.0"
git push origin v0.1.0
```

4. 打开 Actions 里的 **Release** 工作流，等 Windows / macOS 编完。
5. GitHub 上会出现 **draft** Release。下载 zip 自己点一下，再在网页上点 Publish。

预发布用 `v0.1.0-rc.1` 这种 semver，工作流会加上 prerelease 标记。

CI **不会**代你创建或移动 tag。

## 本地打包

Windows（仓库根目录，先编好 release）：

```powershell
cargo build --release -p xmanager-ui -p xmanager-cli
.\scripts\package-windows.ps1 -Version 0.1.0 -Ui target\release\xmanager.exe -Cli target\release\xmanager-cli.exe
```

macOS（需要 Xcode / Command Line Tools，编两个架构再 `lipo`）：

```bash
cargo build --release --target aarch64-apple-darwin -p xmanager-ui -p xmanager-cli
cargo build --release --target x86_64-apple-darwin -p xmanager-ui -p xmanager-cli
./scripts/package-macos.sh --version 0.1.0 \
  --ui-arm target/aarch64-apple-darwin/release/xmanager \
  --ui-x64 target/x86_64-apple-darwin/release/xmanager \
  --cli-arm target/aarch64-apple-darwin/release/xmanager-cli \
  --cli-x64 target/x86_64-apple-darwin/release/xmanager-cli
```

输出在 `dist/`。

## 工作流

| 文件 | 何时跑 |
|------|--------|
| `.github/workflows/ci.yml` | PR，以及 push 到 `main` / `dev` |
| `.github/workflows/release.yml` | 推送 `v*.*.*` tag |

PR CI：`rustfmt`、Windows / macOS 全 workspace 测试、Linux 上只测 `xmanager-core` 和 `xmanager-cli`（桌面端 GPUI 不在 Ubuntu 交叉编译）。

# 发版

`v*` tag 触发、Windows / macOS **各自原生编译**、草稿 Release。发版只认 **`main`**：日常在 `dev` 开发，合进 `main` 后再打 tag。

## 产物

每个 tag 产出：

| 文件 | 内容 |
|------|------|
| `XManager-<ver>-windows-x64.zip` | `XManager.exe` + `xmanager-cli.exe` + `.env.example` + `README.txt` + `LICENSE` |
| `XManager-<ver>-macos-universal.zip` | `XManager.app`（Intel + Apple Silicon）+ `xmanager-cli` + 同上文档 |
| `SHA256SUMS.txt` | 两个 zip 的 SHA-256 |

构建是**未签名**的。Windows 可能出 SmartScreen；macOS 需要 `xattr -cr XManager.app` 或右键打开。zip 内 `README.txt` 有说明。

## 打 tag

1. 把 `[workspace.package] version` 改成要发的版本（例如 `0.1.1`）。
2. 在 `dev` 上提交、推送，等 CI 绿。
3. 把 `dev` 合进 `main` 并推送（快进即可）：

```bash
git checkout main
git merge --ff-only dev
git push origin main
```

4. **tag 必须指向已经在 `origin/main` 上的提交。** 在该提交上打与 Cargo 版本一致的 tag（前缀 `v`）：

```bash
git tag -a v0.1.1 -m "XManager 0.1.1"
git push origin v0.1.1
```

只在 `dev` 上打 tag、还没合 `main`，Release 工作流会拒绝。

5. 打开 Actions 里的 **Release** 工作流，等 Windows / macOS 编完。
6. GitHub 上会出现 **draft** Release。下载 zip 自己点一下，再在网页上点 Publish。

预发布用 `v0.1.1-rc.1` 这种 semver，工作流会加上 prerelease 标记。

CI **不会**代你创建或移动 tag。

## 本地打包

Windows（仓库根目录，先构建前端再编 release）：

```powershell
cd crates\xmanager-tauri\ui; npm install; npm run build; cd ..\..
cargo build --release -p xmanager-tauri --features custom-protocol -p xmanager-cli
.\scripts\package-windows.ps1 -Version 0.1.0 -Ui target\release\xmanager.exe -Cli target\release\xmanager-cli.exe
```

macOS（需要 Xcode / Command Line Tools 与 Node，编两个架构再 `lipo`）：

```bash
(cd crates/xmanager-tauri/ui && npm install && npm run build)
cargo build --release --target aarch64-apple-darwin -p xmanager-tauri --features custom-protocol -p xmanager-cli
cargo build --release --target x86_64-apple-darwin -p xmanager-tauri --features custom-protocol -p xmanager-cli
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
| `.github/workflows/release.yml` | 推送 `v*.*.*` tag（提交须已在 `origin/main`） |

PR CI：`rustfmt`、Windows / macOS 全 workspace 测试、Linux 上只测 `xmanager-core` 和 `xmanager-cli`（Linux 桌面构建需要 WebKitGTK 系统包，CI 未安装）。

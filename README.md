# Rebased (Rust)

> 基于 Rust + gpui 的桌面 Git 客户端 —— 参考 [DetachHead/rebased](https://github.com/DetachHead/rebased)（JetBrains IDE fork，仅保留 Git 集成）的思路用 Rust 重写。
>
> A cross-platform desktop Git client, rewritten in Rust with gpui. Inspired by [DetachHead/rebased](https://github.com/DetachHead/rebased).

[![Release](https://github.com/robertpanvip/git-client/actions/workflows/release.yml/badge.svg)](https://github.com/robertpanvip/git-client/actions/workflows/release.yml)

## 特性 / Features

- **提交图谱** — 多分支提交历史可视化、按作者/时间/关键词过滤
- **分支管理** — 创建/切换/删除/重命名，upstream 跟踪（ahead/behind）
- **Rebase** — 交互式 todo 编辑、`fixup!`/`squash!` 自动重排（autosquash）
- **冲突解析** — 冲突块识别与逐块取舍（ours/theirs/base）
- **代码检查** — blame、文件历史（`--follow`）、跨分支文件对比、Compare（提交级/分支级）
- **工作区** — 暂存/取消暂存、丢弃、stash、amend、空白差异开关
- **标签与远程** — tag 创建/编辑/推送、remote 管理、reflog 视图
- **崩溃兜底** — panic 日志落盘（`~/.local/share/rebased-rs/panic.log`）

底层实现：全部 Git 操作通过系统 `git` CLI（`std::process::Command`）执行，零 `git2` 依赖。

## 下载 / Download

从 [Releases](https://github.com/robertpanvip/git-client/releases/latest) 获取对应平台产物：

| 平台 | 产物 |
|---|---|
| Linux x86_64 | `rebased-rs-linux-x86_64.tar.gz` |
| macOS (Apple Silicon) | `rebased-rs-darwin-aarch64.tar.gz` |
| macOS (Intel) | `rebased-rs-darwin-x86_64.tar.gz` |
| Windows x86_64 | `rebased-rs-windows-x86_64.zip` |

每个平台附 `.sha256` 校验文件。

## 运行依赖 / Requirements

- 系统安装 [git](https://git-scm.com/) 并在 `PATH` 中可用
- 桌面环境：Linux 需 X11 或 Wayland（`libxkbcommon`、fontconfig 等）；macOS / Windows 无额外依赖

```bash
tar -xzf rebased-rs-linux-x86_64.tar.gz
chmod +x rebased-rs-linux-x86_64
./rebased-rs-linux-x86_64 /path/to/repo
```

## 从源码构建 / Build from source

```bash
git clone https://github.com/robertpanvip/git-client.git
cd git-client/rebased-rs
cargo build --release
```

Linux 构建依赖：

```bash
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libfontconfig-dev libfreetype-dev
```

依赖下载使用腾讯镜像加速（rsproxy）时可参考 [rsproxy.cn](https://rsproxy.cn) 的 `~/.cargo/config.toml` 配置。

## 发版 / Releasing

推送 `v*` 标签即自动触发多平台构建并发布 Release：

```bash
git tag v0.x.y && git push origin v0.x.y
```

也可在 [Actions](https://github.com/robertpanvip/git-client/actions/workflows/release.yml) 页面手动触发（仅构建验证，不发布）。

## 项目结构 / Layout

```
.
├── rebased-rs/        # Rust 源码（gpui 桌面应用）
│   ├── src/git/       # Git CLI 封装（ops → repo → backend trait）
│   ├── src/ui/        # gpui 界面
│   └── tests/         # 单元测试 + 真实仓库冒烟测试
└── .github/workflows/ # 多平台构建与发布流水线
```

## License

见 [LICENSE](./LICENSE)。

# Spring Config Converter（OpenIDEA 插件）

Spring Boot 配置三格式互转：YAML / Properties / ENV，遵循 Relaxed Binding 规则。参考 IDEA 版插件 `SpringYamlToEnvConverter` 实现。

## 功能

- 选中配置文本 → 右键菜单「Spring 配置转换」→ 原地替换（支持撤销）
- 无选区时转换整个文档
- 文件右键 / 编辑器标签页右键 → 同目录生成目标格式文件
- 按扩展名自动识别源格式，相同格式不重复转换
- 快捷键：`Alt+Shift+Y` / `Alt+Shift+P` / `Alt+Shift+E`

## 构建与产物

GitHub Actions 在 Linux 环境自动构建标准产物（`.oiplugin`）：

- 每次 push / PR 构建并上传 Artifacts
- 推送 `v*` 标签时附带发布 GitHub Release

本地构建：

```bash
cargo build --release --target wasm32-wasip2
mkdir -p dist
cp target/wasm32-wasip2/release/openidea_plugin_spring_config_converter.wasm dist/plugin.wasm
openidea-plugin build   # 官方 CLI（需 Linux / WSL），构建并校验
```

编译产物不纳入版本控制（见 `.gitignore`）。

## 安装

- **Marketplace**：Settings → Plugins → Available，发布后可在其中搜索 `Spring Config Converter`
- **本地**：下载 Releases 中的 `.oiplugin`，Install Plugin from Disk
- **开发**：在源码目录执行 `openidea-plugin link` 后重载 IDE

## 发布到 Marketplace

社区插件通过 registry [`l0ng-ai/openidea-plugins`](https://github.com/l0ng-ai/openidea-plugins) 收录。条目文件已备在 `registry/` 目录，提交步骤：

1. 在 registry 仓库新建分支，将 `registry/publishers/edc.toml` 放到 `publishers/edc.toml`
2. 将 `registry/plugins/edc.spring-config-converter.toml` 放到 `plugins/edc.spring-config-converter.toml`
3. 提交 PR，等待 maintainer 审核（新发布者与插件会检查源码、权限与签名）
4. 合并后一小时内同步到 OpenIDEA 默认 marketplace

新版本发布流程：本项目打 `vX.Y.Z` 标签 → CI 构建签名包到 Release → 在 registry 的 `plugins/edc.spring-config-converter.toml` 追加 `[[versions]]`（含版本、包 URL、SHA-256）→ 提交 PR。公钥见 `SIGNING_PUBLIC_KEY.md`。

## 转换规则

- ENV 键：大写、`.` 转 `_`，列表用 `_0_` 索引
- Properties 键：点分小写，列表用 `key[0]`
- 连续数字键自动还原为列表，扁平数值/布尔还原为 YAML 原生标量

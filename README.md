# Spring Config Converter（OpenIDEA 插件）

Spring Boot 配置三格式互转：YAML / Properties / ENV，遵循 Relaxed Binding 规则。参考 IDEA 版插件 `SpringYamlToEnvConverter` 实现。

## 功能

- 选中配置文本 → 右键菜单「Spring 配置转换」→ 原地替换（支持撤销）
- 无选区时转换整个文档
- 文件右键 / 编辑器标签页右键 → 同目录生成目标格式文件
- 按扩展名自动识别源格式，相同格式不重复转换
- 快捷键：`Alt+Shift+Y` / `Alt+Shift+P` / `Alt+Shift+E`

## 构建

```bash
cargo build --release --target wasm32-wasip2
mkdir -p dist
cp target/wasm32-wasip2/release/openidea_plugin_spring_config_converter.wasm dist/plugin.wasm
```

或使用官方 CLI（需 Linux / WSL）：

```bash
openidea-plugin build
```

产物为 `dist/plugin.wasm`。`openidea-plugin check` 已通过（插件 API 1.14）。

## 安装

在 Settings → Plugins 中选择本目录，或执行 `openidea-plugin link` 后重载 IDE。

## 转换规则

- ENV 键：大写、`.` 转 `_`，列表用 `_0_` 索引
- Properties 键：点分小写，列表用 `key[0]`
- 连续数字键自动还原为列表，扁平数值/布尔还原为 YAML 原生标量

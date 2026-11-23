# 发布者签名密钥

本插件的发布者 id 为 `edc`，签名公钥（Ed25519，base64）：

```
SkrkGxiXbLIVxNeRslmkkcxZybo1aZJWBxgCmuInibw=
```

私钥保存在 GitHub 仓库 Secret `OPENIDEA_SIGNING_KEY`，CI 构建时临时注入用于签名，不入库。

## 让签名被 IDE 信任

OpenIDEA 通过 marketplace index 验证发布者公钥。要让安装页显示「已签名」而非警告，需要在 index 中注册：

```bash
openidea-plugin index market/ --publisher edc --name edc --key edc.key \
  --download-base https://github.com/edclol/openidea-spring-config-converter/releases/download/v0.1.0
```

然后把 `market/index.json` 与 `.oiplugin` 一起发布到静态 URL，并在 IDE 的 Settings → Plugins → Available 中填入该 index URL。

从磁盘安装时，即使包已签名，IDE 仍会提示未验证发布者（因为没有 index 可核对公钥）；这是 IDE 的安全设计，不影响插件功能。

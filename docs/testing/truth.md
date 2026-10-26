# Truth 真实邮箱测试说明

Truth 测试是 Postium Mail 的手动真实账号测试，用于验证真实 IMAP 服务连接、真实账号添加和基础读取交互。SMTP 发送暂未纳入 truth 断言。

> GPUI 迁移后，Rust/core 层 truth 测试保留并随 workspace 运行；旧 WebdriverIO 版 UI 联测（`e2e/` 的 `real-accounts.e2e.js`）为遗留，UI 层真实联测待按 GPUI 模式重建。

## 适用范围与命令

core 层（真实 IMAP 登录、文件夹读取/选择）：

```bash
cargo test -p postium-core --test real_mail_accounts -- --ignored --nocapture
```

默认 `cargo test --workspace` 不运行 truth（`--ignored` 显式触发）。

旧轨命令（遗留，随 Tauri 轨道移除）：

```bash
bun run test:rust:truth    # 等价于上方 core 层命令（走 src-tauri）
bun run test:e2e:truth     # WebdriverIO UI 联测（遗留）
```

## 配置文件

默认读取仓库根目录 `.test_mail_accounts.json`（已被 `.gitignore` 忽略，不要提交真实凭据）。可用环境变量指定路径：

```bash
POSTIUM_REAL_MAIL=1 POSTIUM_REAL_MAIL_CONFIG=/path/to/accounts.json \
  cargo test -p postium-core --test real_mail_accounts -- --ignored --nocapture
```

配置格式：

```json
{
  "accounts": {
    "gmail": {
      "email": "your_email@gmail.com",
      "password": "your_app_password",
      "imap": { "host": "imap.gmail.com", "port": 993, "ssl": true },
      "smtp": { "host": "smtp.gmail.com", "port": 465, "ssl": true }
    }
  }
}
```

## 运行行为

core 层 truth 遍历所有配置账号，对每个账号执行 IMAP 登录、文件夹列表读取和文件夹选择。不写入、不删除、不发送。

## GPUI 应用手动真实验收（替代旧 e2e truth）

```bash
cargo run -p postium-ui
```

配合设置窗口走真实账号添加（密码型或 OAuth2），覆盖：账号添加 → 首次同步范围选择 → 列表/详情 → 写信。数据目录与生产相同（`~/.postium`），如需隔离请用 `POSTIUM_E2E=1` + `POSTIUM_DATA_DIR` 指向临时目录。

## SMTP 行为

当前 truth 不发送真实邮件。SMTP 配置保留在配置文件中，供发信功能后续扩展 truth 覆盖。

## 失败排查

- core truth 输出中的 account key 与协议阶段；
- 应用日志（tracing 输出，`RUST_LOG=debug` 提升级别）；
- 日志不应出现密码；如发现先修复脱敏再继续。

## 常见问题

- Gmail、QQ、163、iCloud 等服务商通常需要应用专用密码或授权码；
- 需要在邮箱设置中开启 IMAP（SMTP 纳入断言后再要求开启）；
- 服务商可能因风控、限流或异地登录阻止测试。

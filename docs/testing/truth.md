# Truth 真实邮箱测试说明

Truth 测试是 Postium Mail 的手动真实账号测试。当前必过范围是连接真实 IMAP 服务、验证真实账号添加和基础读取交互。SMTP 发送暂未纳入 truth 断言。

## 适用范围

- `bun run test:rust:truth`：验证真实 IMAP 登录和基础读取。
- `bun run test:e2e:truth`：启动真实 Tauri 应用，从空数据目录通过 UI 添加真实账号、首次同步、切换账号、查看列表/详情或空状态。

默认 `bun run test:rust`、`bun run test:e2e` 和 `bun run test:ci` 不运行 truth 测试。

## 配置文件

默认读取仓库根目录 `.test_mail_accounts.json`。也可以指定路径：

```bash
POSTIUM_REAL_MAIL_CONFIG=/path/to/accounts.json bun run test:rust:truth
POSTIUM_REAL_MAIL_CONFIG=/path/to/accounts.json bun run test:e2e:truth
```

配置格式：

```json
{
  "accounts": {
    "gmail": {
      "email": "your_email@gmail.com",
      "password": "your_app_password",
      "imap": {
        "host": "imap.gmail.com",
        "port": 993,
        "ssl": true
      },
      "smtp": {
        "host": "smtp.gmail.com",
        "port": 465,
        "ssl": true
      }
    }
  }
}
```

`.test_mail_accounts.json` 已被 `.gitignore` 忽略，不要提交真实凭据。

## 运行 Rust Truth

```bash
bun run test:rust:truth
```

该命令会遍历所有配置账号。每个账号会执行 IMAP 登录、文件夹读取和文件夹选择。

## 运行 E2E Truth

```bash
bun run test:e2e:truth
```

该命令使用 `.e2e-truth-data/run-*` 作为独立数据目录，不使用默认 seed 数据。测试成功会清理本次数据目录；失败会保留数据目录和 artifacts。

## SMTP 行为

当前 truth 测试不会发送真实邮件。SMTP 配置仍保留在 `.test_mail_accounts.json` 中，供后续发信功能实现后扩展 truth 覆盖。

## 失败排查

优先查看：

- Rust truth 输出中的 account key 和协议阶段。
- `e2e/artifacts/screenshots`
- `e2e/artifacts/reports`
- `e2e/artifacts/logs`
- `.e2e-truth-data/run-*`

日志不应输出密码。如果发现密码出现在日志或 artifacts 中，应先修复脱敏问题，再继续运行 truth。

## 常见问题

- Gmail、QQ、163、iCloud 等服务商通常需要应用专用密码或授权码。
- 需要在邮箱设置中开启 IMAP。SMTP 后续纳入 truth 发送断言时，再要求开启 SMTP。
- 服务商可能因为风控、限流或异地登录阻止测试。

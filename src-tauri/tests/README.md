# Postium Mail 测试模块

## 目录结构

```
tests/
├── integration/                # 集成测试（GreenMail）
│   ├── mod.rs                 # GreenMail 配置
│   ├── helpers.rs             # 测试辅助函数
│   ├── imap/                  # IMAP 协议测试
│   ├── sync/                  # 同步引擎测试
│   ├── engine/                # FlowEngine 测试
│   └── e2e/                   # 端到端测试
│
└── provider/                  # 服务商测试（真实账号）
    ├── mod.rs                 # 测试账号配置
    ├── config.rs              # 环境变量配置
    ├── helpers.rs             # 测试辅助函数
    ├── personal/              # 个人邮箱测试
    │   ├── gmail.rs
    │   ├── outlook.rs
    │   └── native.rs
    └── enterprise/            # 企业邮箱测试
        ├── microsoft_365.rs
        └── google_workspace.rs
```

---

## 集成测试 (integration/)

使用 GreenMail 测试服务器进行协议和逻辑集成测试。

### GreenMail 配置

**公网服务器**（默认）：
- 主机: `139.59.228.56`
- IMAP 端口: `3143`
- SMTP 端口: `3025`
- 用户名: `testuser`
- 密码: `testpass`

**本地 Docker**：
```bash
docker-compose -f docker-compose.test.yml up -d
```

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GREENMAIL_HOST` | GreenMail 主机地址 | `139.59.228.56` |
| `GREENMAIL_PORT` | IMAP 端口 | `3143` |

### 运行方式

```bash
# 进入 src-tauri 目录
cd src-tauri

# 运行所有集成测试
cargo test --test integration

# 运行特定测试
cargo test --test integration test_greenmail_config --

# 查看测试输出
cargo test --test integration -- --nocapture

# 运行被忽略的测试（需要本地 GreenMail）
cargo test --test integration -- --ignored
```

### 测试列表

#### IMAP 协议测试
- `test_greenmail_config` - GreenMail 配置测试
- `test_greenmail_running` - 检查服务器状态

#### 同步引擎测试
- `test_create_test_db` - 创建测试数据库
- `test_init_test_db` - 初始化数据库结构

#### 端到端测试
- `test_greenmail_end_to_end_config` - 端到端配置测试
- `test_database_workflow` - 数据库工作流测试

---

## 服务商测试 (provider/)

使用真实账号进行服务商兼容性测试。

### 环境变量配置

#### Gmail
```bash
export GMAIL_EMAIL="your_email@gmail.com"
export GMAIL_APP_PASSWORD="your_app_password"
```

#### Outlook
```bash
export OUTLOOK_EMAIL="your_email@outlook.com"
export OUTLOOK_APP_PASSWORD="your_app_password"
```

#### 163 邮箱
```bash
export EMAIL_163_ADDR="your_email@163.com"
export EMAIL_163_PASS="your_password"
```

#### QQ 邮箱
```bash
export EMAIL_QQ_ADDR="your_email@qq.com"
export EMAIL_QQ_PASS="your_password"
```

### 运行方式

```bash
# 进入 src-tauri 目录
cd src-tauri

# 运行所有服务商测试
cargo test --test provider

# 运行特定服务商测试
cargo test --test provider test_gmail --

# 列出所有测试
cargo test --test provider -- --list

# 查看测试输出
cargo test --test provider -- --nocapture
```

### 测试列表

#### 个人邮箱测试
- `test_gmail_config_load` - Gmail 配置加载
- `test_gmail_imap_connection` - Gmail IMAP 连接
- `test_outlook_config_load` - Outlook 配置加载
- `test_163_config_load` - 163 配置加载
- `test_qq_config_load` - QQ 配置加载

---

## Docker Compose 配置

使用本地 GreenMail 服务器：

```yaml
# docker-compose.test.yml
services:
  greenmail:
    image: greenmail/standalone:2.0.1
    ports:
      - "3143:143"  # IMAP
      - "3993:993"  # IMAPS
      - "3025:25"   # SMTP
      - "3465:465"  # SMTPS
      - "8080:8080" # Web UI
    environment:
      - GREENMAIL_OPTS=-Dgreenmail.setup.test.all
      - Dgreenmail.user=testuser:testpass
```

启动命令：
```bash
docker-compose -f docker-compose.test.yml up -d
```

查看日志：
```bash
docker logs -f postmium-greenmail
```

停止服务：
```bash
docker-compose -f docker-compose.test.yml down
```

---

## CI/CD 集成

### GitHub Actions 示例

```yaml
name: Tests

on: [push, pull_request]

jobs:
  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Start GreenMail
        run: docker-compose -f docker-compose.test.yml up -d

      - name: Run tests
        run: cargo test --test integration

      - name: Stop GreenMail
        if: always()
        run: docker-compose -f docker-compose.test.yml down

  provider-tests:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    env:
      GMAIL_EMAIL: ${{ secrets.GMAIL_EMAIL }}
      GMAIL_APP_PASSWORD: ${{ secrets.GMAIL_APP_PASSWORD }}
    steps:
      - uses: actions/checkout@v3
      - name: Run provider tests
        run: cargo test --test provider
```

---

## 故障排查

### GreenMail 连接失败

1. 检查服务器是否运行：
   ```bash
   curl http://localhost:8080/health
   ```

2. 检查端口是否被占用：
   ```bash
   netstat -an | grep 3143
   ```

3. 查看服务器日志：
   ```bash
   docker logs postmium-greenmail
   ```

### 服务商测试被跳过

确保设置了正确的环境变量：

```bash
# 检查环境变量
echo $GMAIL_EMAIL
echo $GMAIL_APP_PASSWORD

# 如果为空，需要先设置
export GMAIL_EMAIL="your_email@gmail.com"
export GMAIL_APP_PASSWORD="your_app_password"
```

---

## 最佳实践

1. **提交前运行测试**：确保所有测试通过
2. **使用 `#[ignore]`**：标记需要外部依赖的测试
3. **添加文档**：为测试添加清晰的文档字符串
4. **保持测试独立**：每个测试应该独立运行
5. **使用辅助函数**：避免重复代码

---

## 相关文档

- [架构设计文档](../docs/architecture-design.md)
- [实现计划](../docs/implementation-plan.md)
- [FlowEngine 设计](../docs/stages/mail-flow-engine-design.md)

# 阶段 4 集成测试指南

## 概述

本文档描述如何使用 Docker GreenMail 进行 IMAP 集成测试。

## 为什么需要 GreenMail？

### 问题背景

Rust 的 IMAP 测试面临以下挑战：

1. **缺少可靠的 Mock 库**：
   - Java 有 `mockito`，Python 有 `imap-tools`
   - Rust 的 `mockito` 是 HTTP mock 库，不支持 IMAP 协议
   - IMAP 协议复杂，手写 Mock 成本高

2. **真实服务器测试的问题**：
   - **速率限制**：Gmail/Outlook 有严格的 API 限制
   - **网络依赖**：测试需要网络连接
   - **数据污染**：测试可能影响真实邮件数据
   - **CI/CD 困难**：CI 环境无法访问真实账号

3. **单元测试的局限**：
   - 无法验证完整的数据流
   - 无法测试协议兼容性
   - 无法发现集成问题

### GreenMail 的优势

| 特性 | GreenMail | 真实服务器 | 单元 Mock |
|------|-----------|-----------|----------|
| **协议支持** | ✅ 完整 IMAP | ✅ 完整 IMAP | ⚠️ 部分 |
| **CONDSTORE** | ✅ 支持 | ✅ 支持 | ❌ 很难实现 |
| **数据隔离** | ✅ 完全隔离 | ❌ 共享数据 | ✅ 完全隔离 |
| **CI/CD 友好** | ✅ Docker 化 | ❌ 需要凭据 | ✅ 纯代码 |
| **性能测试** | ✅ 可控 | ❌ 不稳定 | ❌ 不真实 |
| **设置成本** | ⭐⭐ 低 | ⭐⭐⭐⭐⭐ 高 | ⭐⭐⭐⭐ 高 |

## 快速开始

### 1. 启动 GreenMail

```bash
# 启动服务（后台运行）
docker-compose -f docker-compose.test.yml up -d

# 查看日志，确认服务就绪
docker logs -f postmium-greenmail

# 应该看到类似输出：
# GreenMail standalone ... started
```

### 2. 运行集成测试

```bash
# 进入 src-tauri 目录
cd src-tauri

# 运行所有集成测试
cargo test --test integration -- --ignored --nocapture

# 运行特定测试
cargo test test_greenmail_connection -- --ignored --nocapture
```

### 3. 停止 GreenMail

```bash
docker-compose -f docker-compose.test.yml down
```

## GreenMail 配置

### 默认配置

```yaml
docker-compose.test.yml:
  - IMAP 端口: 3143 (映射到容器 143)
  - IMAPS 端口: 3993 (映射到容器 993)
  - SMTP 端口: 3025 (映射到容器 25)
  - Web UI: http://localhost:8080
```

### 测试账号

```bash
用户名: testuser
密码: testpass
邮箱: testuser@localhost
```

### 自定义账号

修改 `docker-compose.test.yml`:

```yaml
environment:
  - GREENMAIL_USER=user1:pass1,user2:pass2
```

## 测试场景

### 场景 1: 基础连接测试

**文件**: `greenmail_sync_test.rs::test_greenmail_connection`

**测试内容**:
- ✅ IMAP 连接
- ✅ 登录认证
- ✅ 列出文件夹
- ✅ 选择 INBOX

**预期结果**:
```
✅ IMAP 连接成功
📁 文件夹列表: ["INBOX"]
✅ 选择 INBOX 成功
```

---

### 场景 2: CONDSTORE 支持检测

**文件**: `greenmail_sync_test.rs::test_greenmail_condstore_support`

**测试内容**:
- ✅ CAPABILITY 命令
- ✅ CONDSTORE 扩展检测

**预期结果**:
```
CONDSTORE 支持: true/false
```

**注意**: GreenMail 1.6.0 可能不支持 CONDSTORE，这是预期的。

---

### 场景 3: 文件夹同步

**文件**: `greenmail_sync_test.rs::test_greenmail_folder_sync`

**测试内容**:
- ✅ 同步文件夹列表
- ✅ 检测 RFC 6154 Special-Use
- ✅ 保存到数据库

**预期结果**:
```
✅ 文件夹同步完成
   新建文件夹: 1
   更新文件夹: 0
📊 数据库中的文件夹数量: 1
✅ INBOX 已同步: INBOX
```

---

### 场景 4: 首次完整同步

**文件**: `greenmail_sync_test.rs::test_greenmail_first_sync`

**测试内容**:
- ✅ 完整同步流程
- ✅ 邮件获取和解析
- ✅ 数据库存储

**预期结果**:
```
✅ 同步成功
   新邮件: N
   更新邮件: 0
   删除邮件: 0
```

---

### 场景 5: 增量同步

**文件**: `greenmail_sync_test.rs::test_greenmail_incremental_sync`

**测试内容**:
- ✅ 首次同步
- ✅ 增量同步（变更检测）
- ✅ 状态更新

**预期结果**:
```
✅ 增量同步完成
   新邮件: 0 (无变更)
   更新邮件: 0
```

---

### 场景 6: DeltaSync 策略

**文件**: `greenmail_sync_test.rs::test_delta_sync_condstore_strategy`

**测试内容**:
- ✅ CONDSTORE 支持检测
- ✅ 策略自动选择
- ✅ 增量同步执行

**预期结果**:
```
CONDSTORE 支持: true/false
✅ DeltaSync 完成
   策略: Condstore/UidSearch
   耗时: XXX ms
```

## 添加新测试

### 步骤 1: 定义测试函数

```rust
// src-tauri/tests/integration/greenmail_sync_test.rs

#[tokio::test]
#[ignore] // 标记为需要手动运行
async fn test_your_new_scenario() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    // 你的测试逻辑...
}
```

### 步骤 2: 运行测试

```bash
cargo test test_your_new_scenario -- --ignored --nocapture
```

## 故障排查

### 问题 1: GreenMail 无法启动

**症状**:
```
Error: bind: address already in use
```

**解决方案**:
```bash
# 检查端口占用
netstat -ano | findstr :3143

# 停止旧容器
docker-compose -f docker-compose.test.yml down

# 或修改端口
# 编辑 docker-compose.test.yml，改用其他端口
```

---

### 问题 2: 测试超时

**症状**:
```
test test_greenmail_connection has been running for over 60 seconds
```

**解决方案**:
```bash
# 检查 GreenMail 日志
docker logs postmium-greenmail

# 重启 GreenMail
docker-compose -f docker-compose.test.yml restart

# 等待服务就绪后再运行测试
sleep 5
cargo test --test integration -- --ignored
```

---

### 问题 3: 认证失败

**症状**:
```
Error: AUTHENTICATIONFAILED
```

**解决方案**:
```rust
// 确认用户名密码正确
const GREENMAIL_USER: &str = "testuser";
const GREENMAIL_PASS: &str = "testpass";

// 或使用默认 GreenMail 账号
// 用户名: test1@localhost
// 密码: test1
```

---

### 问题 4: 测试被跳过

**症状**:
```
⚠️  GreenMail 未运行，跳过测试
```

**解决方案**:
```bash
# 确认 GreenMail 正在运行
docker ps | grep greenmail

# 如果没有运行，启动它
docker-compose -f docker-compose.test.yml up -d
```

## CI/CD 集成

### GitHub Actions 示例

```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest

    services:
      greenmail:
        image: greenmail/standalone:1.6.0
        ports:
          - 3143:143
          - 8080:8080
        options: >-
          --health-cmd "curl -f http://localhost:8080/health || exit 1"
          --health-interval 5s
          --health-timeout 3s
          --health-retries 5

    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run integration tests
        run: |
          cd src-tauri
          cargo test --test integration -- --ignored
        env:
          GREENMAIL_HOST: localhost
          GREENMAIL_PORT: 3143
```

## 性能基准测试

### 运行性能测试

```bash
# 运行性能基准测试
cargo test test_benchmark_sync -- --ignored --nocapture
```

### 预期性能指标

| 操作 | 目标 | 当前状态 |
|------|------|---------|
| IMAP 连接 | < 100ms | ⏳ 待测试 |
| 文件夹列表 | < 200ms | ⏳ 待测试 |
| 首次同步 (10 封邮件) | < 2s | ⏳ 待测试 |
| 增量同步 (0 变更) | < 500ms | ⏳ 待测试 |
| CONDSTORE 同步 | 比 UID 快 30%+ | ⏳ 待验证 |

## 下一步

1. **完善 CONDSTORE 测试**：
   - GreenMail 1.6.0 不支持 CONDSTORE
   - 考虑升级到支持 CONDSTORE 的版本
   - 或使用真实服务器进行 CONDSTORE 测试

2. **添加更多测试场景**：
   - 断线重连
   - 大文件夹性能
   - 并发同步
   - 错误恢复

3. **性能基准测试**：
   - 对比 CONDSTORE vs UID 搜索
   - 测试不同文件夹大小
   - 内存占用监控

4. **真实服务器测试**：
   - Gmail CONDSTORE 测试
   - Outlook 降级测试
   - QQ 邮箱兼容性测试

---

**文档版本**: v1.0
**最后更新**: 2026-03-18
**维护者**: Postium Mail Team

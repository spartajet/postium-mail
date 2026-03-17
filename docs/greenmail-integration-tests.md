# GreenMail 集成测试快速指南

## 概述

本指南介绍如何使用 Docker GreenMail 进行 IMAP 集成测试。

## 快速开始

### Windows 用户

```powershell
# 启动 GreenMail 并运行测试
.\scripts\run-integration-tests.bat

# 运行特定测试
.\scripts\run-integration-tests.bat greenmail_connection

# 测试后保持 GreenMail 运行
.\scripts\run-integration-tests.bat --no-stop
```

### Linux/macOS 用户

```bash
# 添加执行权限
chmod +x scripts/run-integration-tests.sh

# 启动 GreenMail 并运行测试
./scripts/run-integration-tests.sh

# 运行特定测试
./scripts/run-integration-tests.sh greenmail_connection

# 测试后保持 GreenMail 运行
./scripts/run-integration-tests.sh --no-stop
```

## 手动操作

### 1. 启动 GreenMail

```bash
docker-compose -f docker-compose.test.yml up -d
```

### 2. 运行测试

```bash
cd src-tauri
cargo test --test mod integration:: -- --ignored --nocapture
```

### 3. 停止 GreenMail

```bash
docker-compose -f docker-compose.test.yml down
```

## 可用的测试

| 测试名称 | 描述 | 需求 |
|---------|------|------|
| `test_greenmail_running` | 检查 GreenMail 状态 | GreenMail |
| `test_greenmail_connection` | 基本 IMAP 连接 | GreenMail |
| `test_tcp_connection_to_greenmail` | TCP 连接和认证 | GreenMail |
| `test_docker_compose_file` | 验证 Docker 配置 | 无 |
| `test_integration_test_structure` | 验证测试文件结构 | 无 |
| `test_documentation_files` | 验证文档完整性 | 无 |

## 测试结果

```
running 6 tests
test integration::greenmail_sync_test::test_greenmail_running ... ok
test integration::greenmail_sync_test::test_greenmail_connection ... ok
test integration::greenmail_sync_test::test_tcp_connection_to_greenmail ... ok
test integration::greenmail_sync_test::test_docker_compose_file ... ok
test integration::greenmail_sync_test::test_integration_test_structure ... ok
test integration::greenmail_sync_test::test_documentation_files ... ok

test result: ok. 6 passed; 0 failed
```

## 故障排查

### 问题：端口已被占用

**症状**：
```
Error: bind: address already in use
```

**解决方案**：
```bash
# 检查端口占用
netstat -ano | findstr :3143  # Windows
netstat -tuln | grep 3143     # Linux/macOS

# 停止旧容器
docker-compose -f docker-compose.test.yml down

# 或修改端口（编辑 docker-compose.test.yml）
```

### 问题：GreenMail 启动超时

**症状**：
```
GreenMail 启动超时
```

**解决方案**：
```bash
# 查看 GreenMail 日志
docker logs postmium-greenmail

# 重启 GreenMail
docker-compose -f docker-compose.test.yml restart

# 等待服务就绪
curl http://localhost:8080/health
```

### 问题：测试被跳过

**症状**：
```
⚠️  GreenMail 未运行，跳过测试
```

**解决方案**：
```bash
# 确认 GreenMail 正在运行
docker ps | grep greenmail

# 如果没有运行，启动它
docker-compose -f docker-compose.test.yml up -d
```

## GreenMail Web UI

GreenMail 提供了一个 Web UI 来查看和管理测试邮件：

- **URL**: http://localhost:8080
- **功能**:
  - 查看所有测试账号的邮件
  - 发送测试邮件
  - 查看邮件内容

## 下一步

1. ✅ 基础连接测试已完成
2. ⏳ 添加邮件同步集成测试
3. ⏳ 添加 CONDSTORE 功能测试
4. ⏳ 添加性能基准测试

## 相关文档

- [完整集成测试指南](./integration-testing-guide.md)
- [阶段 4 最终评估报告](./stage4-final-assessment.md)
- [Docker Compose 配置](../docker-compose.test.yml)

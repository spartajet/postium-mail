# 集成测试 README

## 概述

本目录包含 Postium Mail 的集成测试。

## 目录结构

```
tests/
├── integration/           # 集成测试
│   ├── mod.rs
│   └── greenmail_sync_test.rs
└── README.md             # 本文件
```

## 运行测试

### 前置条件

1. 安装 Docker 和 Docker Compose
2. 克隆仓库并进入项目根目录

### 启动测试服务器

```bash
# 在项目根目录执行
docker-compose -f docker-compose.test.yml up -d

# 查看服务状态
docker-compose -f docker-compose.test.yml ps

# 查看日志
docker logs -f postmium-greenmail
```

### 运行集成测试

```bash
# 进入 src-tauri 目录
cd src-tauri

# 运行所有集成测试
cargo test --test integration -- --ignored --nocapture

# 运行特定测试
cargo test test_greenmail_connection -- --ignored --nocapture
```

### 清理

```bash
# 停止并删除容器
docker-compose -f docker-compose.test.yml down

# 删除数据卷（如果需要）
docker-compose -f docker-compose.test.yml down -v
```

## 测试覆盖

### GreenMail 集成测试

| 测试 | 描述 | 状态 |
|------|------|------|
| `test_greenmail_connection` | IMAP 连接测试 | ✅ 已实现 |
| `test_greenmail_condstore_support` | CONDSTORE 支持检测 | ✅ 已实现 |
| `test_greenmail_folder_sync` | 文件夹同步 | ✅ 已实现 |
| `test_greenmail_first_sync` | 首次完整同步 | ✅ 已实现 |
| `test_greenmail_incremental_sync` | 增量同步 | ✅ 已实现 |
| `test_delta_sync_condstore_strategy` | DeltaSync 策略 | ✅ 已实现 |

## 故障排查

详见 [集成测试指南](../../docs/integration-testing-guide.md)

## 相关文档

- [集成测试指南](../../docs/integration-testing-guide.md)
- [阶段4最终评估报告](../../docs/stage4-final-assessment.md)

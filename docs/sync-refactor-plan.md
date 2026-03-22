# 邮件同步系统重构计划

## 背景

根据同步流程设计文档，需要重构现有同步代码以实现：
1. **能力探测架构** - 根据 IMAP CAPABILITY 动态选择同步策略
2. **双策略支持** - CONDSTORE 增量同步 + Fallback 全量比对
3. **完整 flags 支持** - 支持所有 IMAP 系统标志，而非仅 3 个布尔字段
4. **UIDVALIDITY 处理** - 邮箱重置检测与状态重置

---

## 禂览

### 已完成 ✅

| Phase | 描述 | 綆认 |
|-------|------|------|
| Phase 1 | Email 模型扩展 | ✅ 添加 `is_answered`/`is_deleted` 字段，| Phase 2 | 能力探测集成 | ✅ 已集成到 `sync_manager.rs` |
| Phase 3 | UIDVALIDITY 处理 | ✅ 添加检测和重置方法 |
| Phase 4 | Flags 批量更新方法 | ✅ 实现 `batch_update_flags()` |
| Phase 5 | change_detector 适配 | ✅ 更新 `from_email_model()` |

### 待实现 ❌

| Phase | 描述 | 备注 |
|-------|------|------|
| Phase 5 (QRESYNC) | VANISHED 响应解析 | 可选优化 |

---

## 已实现组件详情

### Phase 1: Email 模型扩展

**修改文件**:
1. `src-tauri/src/storage/models/email.rs` - 添加 `is_answered` 和 `is_deleted` 字段
2. `src-tauri/src/storage/migration/m012_20250322_add_email_flags.rs` - 数据库迁移

3. `src-tauri/src/sync/change_detector.rs` - 更新 `from_email_model()`
4. `src-tauri/src/sync/mail_processor.rs` - 插入/更新时设置新字段

**实现方案**:
- 使用布尔字段而非 JSON 存储（更简洁，查询性能更好）
- 新增字段：`is_answered` (\\Answered) 和 `is_deleted` (\\Deleted)
- 保留原有字段：`is_read`, `is_starred`, `is_draft`

```rust
pub struct Model {
    // ... 现有字段 ...
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub is_answered: bool,  // 新增
    pub is_deleted: bool,  // 新增
}
```

### Phase 2: 能力探测集成

**状态**: ✅ 已完成

`client.rs:1100-1129` 中的 `check_condstore_support()` 已被 `sync_manager.rs:356-359` 直接调用：

```rust
// sync_manager.rs
let supports_condstore = imap_client
    .check_condstore_support()
    .await
    .unwrap_or(false);
```

### Phase 3: UIDVALIDITY 处理

**修改文件**: `src-tauri/src/sync/folder_manager.rs`

**新增方法**:
- `check_uidvalidity_changed()` - 检测 UIDVALIDITY 变化
- `reset_sync_state()` - 重置文件夹同步状态

**集成位置**: `sync_manager.rs` 在 `sync_folder_internal()` 中调用

```rust
// 检测 UIDVALIDITY 变化
let uidvalidity_changed = self.folder_manager
    .check_uidvalidity_changed(account_id, folder, meta.uidvalidity)
    .await?;

if uidvalidity_changed {
    self.folder_manager.reset_sync_state(account_id, folder).await?;
    needs_full_resync = true;
}
```

---

## 已有实现可直接使用 ✅

| 组件 | 文件位置 | 说明 |
|------|----------|------|
| `check_condstore_support()` | `client.rs:1100-1129` | ✅ 已完整实现 |
| `select_with_condstore()` | `client.rs:1176-1210` | ✅ 已完整实现 |
| `search_modified_since()` | `client.rs:1260-1266` | ✅ 已完整实现 |
| `fetch_with_modseq()` | `client.rs:1308-1325` | ✅ 已完整实现 |
| `fetch_modseqs()` | `client.rs:1369-1381` | ✅ 已完整实现 |

---

## 待实现组件

### Phase 5: QRESYNC 支持（可选）

**优先级**: P3（高级优化）

**说明**: 支持 QRESYNC 扩展以处理 VANISHED 响应

---

## 关键文件清单

| 文件路径 | 修改类型 | 状态 |
|----------|----------|------|
| `src-tauri/src/storage/models/email.rs` | 添加 is_answered/is_deleted | ✅ 完成 |
| `src-tauri/src/storage/migration/m012_20250322_add_email_flags.rs` | 新增迁移 | ✅ 完成 |
| `src-tauri/src/sync/change_detector.rs` | 适配 flags 字段 | ✅ 完成 |
| `src-tauri/src/sync/mail_processor.rs` | 设置新字段 | ✅ 完成 |
| `src-tauri/src/sync/sync_manager.rs` | 集成能力探测、UIDVALIDITY | ✅ 完成 |
| `src-tauri/src/sync/folder_manager.rs` | UIDVALIDITY 检测/重置 | ✅ 完成 |
| `src-tauri/src/storage/service/email.rs` | batch_update_flags | ✅ 完成 |

---

## 验证计划

### 编译验证
```bash
cargo check --package postium-mail
```

### 单元测试
```bash
cargo test --package postium-mail --lib sync::
```

1. 测试 `EmailFlags` 序列化/反序列化
2. 测试 UIDVALIDITY 变化检测
3. 测试 Fallback 策略的 flags 比对

### 集成测试
1. Gmail 账号 - 验证 CONDSTORE 增量同步
2. 163/QQ 账号 - 验证 Fallback 全量比对
3. 模拟 UIDVALIDITY 变化

### 手动验证
1. 修改服务器邮件 flags，验证本地同步更新
2. 在服务器删除邮件，验证本地删除
3. 首次同步大量邮件，验证性能

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 数据库迁移失败 | 高 | 先备份数据，添加回滚脚本 |
| Fallback 策略性能差 | 中 | 使用分批处理，添加进度反馈 |
| Flags 解析错误 | 中 | 详细错误日志，支持部分失败继续 |
| 向后兼容性 | 中 | 保留原有布尔字段，新字段为补充 |

---

## 完成记录

- **2025-03-22**: Phase 1-3, 5 完成
  - 添加 `is_answered` 和 `is_deleted` 字段到 Email 模型
  - 创建数据库迁移 m012
  - 更新 `change_detector.rs` 和 `mail_processor.rs` 使用新字段
  - 在 `folder_manager.rs` 添加 UIDVALIDITY 检测和重置方法
  - 在 `sync_manager.rs` 集成 UIDVALIDITY 检测

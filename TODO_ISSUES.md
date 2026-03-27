# TODO 转换为 GitHub Issue 清单

本文档记录代码清理过程中发现的、需要转换为 GitHub Issue 的 TODO 项。

## 创建 Issue 指南

为每个 TODO 创建 GitHub Issue 时，请包含以下信息：
- **标题**: 使用简洁的描述性标题
- **标签**: 添加 `enhancement`、`priority: low` 等标签
- **描述**: 复制下方 TODO 的详细描述
- **所属模块**: 标注影响的代码模块

---

## 1. 测试相关 Issue

### 1.1 AuthManager 集成测试
**文件**: `src-tauri/src/auth/auth_manager.rs:893`
```rust
// TODO: 实现实际的集成测试
```
**建议 Issue 标题**: `[测试] 为 AuthManager 添加集成测试`
**描述**:
- 测试 OAuth2 认证流程
- 测试密码认证流程
- 测试 Token 刷新机制
- 测试账号 ID 更新流程
- 测试 Keyring 存储集成

### 1.2 PasswordAuth 集成测试
**文件**: `src-tauri/src/auth/credentials/password_auth.rs:201,208`
```rust
// TODO: 实现实际的集成测试
```
**建议 Issue 标题**: `[测试] 为 PasswordAuth 添加集成测试`
**描述**:
- 测试密码验证流程
- 测试错误处理（密码错误、账号不存在等）
- 测试 IMAP/SMTP 密码认证集成

### 1.3 企业认证 MFA 和条件访问检测
**文件**: `src-tauri/src/auth/enterprise_auth.rs:148,173`
```rust
// TODO: 实现实际的 MFA 检测
// TODO: 实现实际的条件访问检测
```
**建议 Issue 标题**: `[功能] 实现企业认证 MFA 和条件访问检测`
**描述**:
- 检测 Microsoft 365 的 MFA 要求
- 检测条件访问策略（设备信任、位置等）
- 提示用户使用 OAuth2 而非密码认证

### 1.4 FlowEngine 测试用例
**文件**: `src-tauri/src/engine/flow_engine.rs:213`
```rust
// TODO: 添加实际的测试用例
```
**建议 Issue 标题**: `[测试] 为 FlowEngine 添加单元测试和集成测试`
**描述**:
- 测试任务调度器
- 测试 IDLE 监听管理
- 测试通知管理器
- 测试引擎生命周期

---

## 2. 功能增强 Issue

### 2.1 通知插件集成
**文件**: `src-tauri/src/engine/notification_manager.rs:410`
```rust
// TODO: 集成 tauri-plugin-notification
```
**建议 Issue 标题**: `[功能] 集成 tauri-plugin-notification 实现桌面通知`
**描述**:
- 安装并配置 `tauri-plugin-notification`
- 实现新邮件通知
- 实现同步错误通知
- 支持用户自定义通知设置
- 关联前端 TODO: `src/stores/email.ts:596`

### 2.2 MX 记录查询
**文件**: `src-tauri/src/providers/provider_pool.rs:150,176,227,237`
```rust
// TODO: 实现 MX 记录查询
// TODO: 实际查询 MX 记录
// TODO: 这里应该查询实际的 MX 记录
```
**建议 Issue 标题**: `[功能] 实现基于 MX 记录的邮箱服务商自动检测`
**描述**:
- 查询域名的 MX 记录
- 根据 MX 记录推断邮箱服务商
- 支持自定义域名的企业邮箱
- 改进 `detect_provider` 功能的准确性
**技术要点**:
- 使用 `trust-dns` 或类似库查询 DNS
- 缓存 MX 记录以提升性能
- 处理查询失败的超时和错误

### 2.3 搜索历史功能
**文件**: `src-tauri/src/storage/search.rs:10,166,270`
```rust
//! - **搜索历史**: 热门搜索关键词（TODO）
// TODO: 未来考虑使用 sea-orm 的 from_raw_sql 配合 Entity 来提升安全性
/// TODO: 实现搜索历史记录功能
```
**建议 Issue 标题**: `[功能] 实现搜索历史记录功能`
**描述**:
- 记录用户搜索关键词
- 显示热门搜索
- 支持删除搜索历史
- 提供搜索建议
**数据库设计**:
- 创建 `search_history` 表
- 字段：`id`, `account_id`, `keyword`, `search_count`, `last_searched_at`
- 为 `keyword` 和 `search_count` 创建索引

---

## 3. 前端功能 Issue

### 3.1 编辑账号模态框
**文件**: `src/components/settings/SettingsModal.vue:178`
```typescript
// TODO: 打开编辑账号模态框
```
**建议 Issue 标题**: `[功能] 实现编辑账号功能`
**描述**:
- 创建编辑账号模态框组件
- 支持修改账号名称、颜色
- 支持修改服务器配置（IMAP/SMTP）
- 支持修改同步设置
- 验证服务器配置变更

### 3.2 工作流运行逻辑
**文件**: `src/components/workflow/WorkflowView.vue:212`
```typescript
// TODO: 实现工作流运行逻辑
```
**建议 Issue 标题**: `[功能] 实现工作流执行引擎`
**描述**:
- 定义工作流 DSL 或配置格式
- 实现工作流解析器
- 实现工作流执行器
- 支持常用邮件操作（过滤、分类、转发等）
- 提供工作流模板
**优先级**: 低（这是一个高级功能）

---

## 统计信息

- **测试相关**: 4 个 Issue
- **功能增强**: 3 个 Issue
- **前端功能**: 2 个 Issue
- **总计**: 9 个 Issue

## 优先级建议

### 高优先级
1. 通知插件集成（提升用户体验）
2. 编辑账号功能（基础功能完善）

### 中优先级
1. 集成测试（提升代码质量）
2. MX 记录查询（改进自动化）

### 低优先级
1. 搜索历史（锦上添花）
2. 工作流执行引擎（高级功能）
3. MFA 检测（可通过文档说明）

---

## 注意事项

1. **创建 Issue 前**：先搜索是否已存在类似 Issue
2. **Issue 创建后**：在代码中将 TODO 替换为 Issue 链接，例如：
   ```rust
   // TODO: 实现 MX 记录查询
   // 见: https://github.com/xxx/postium-mail/issues/123
   ```
3. **定期审查**：每个版本发布前审查这个清单
4. **完成清理**：Issue 完成后，删除对应的 TODO 注释

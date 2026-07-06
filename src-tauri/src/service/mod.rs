// ═══════════════════════════════════════════════════════════════
// Service 层模块
// ═══════════════════════════════════════════════════════════════
//
// 本模块定义了应用的业务逻辑层，实现了以下功能：
// 1. 账号管理 - 创建、更新、删除账号，处理密码和 OAuth2 认证
// 2. 邮件管理 - 邮件的列表、详情、搜索、发送、删除等操作
// 3. 标签管理 - 自定义标签的创建、更新、删除
// 4. 同步管理 - 与邮件服务器的同步操作
//
// 设计原则：
// - Service 层负责业务逻辑编排，不直接处理数据库操作
// - 数据持久化委托给 Infrastructure 层的 repository
// - 认证相关逻辑委托给 Domain 层的 AuthManager
// - 通过 DTO (Data Transfer Object) 与 Command 层进行数据交换
// ═══════════════════════════════════════════════════════════════

// ───── 子模块声明 ─────

/// 账号服务模块
///
/// 职责：
/// - 账号的增删改查（CRUD）操作
/// - 账号密码的存储和更新（使用系统 keyring）
/// - OAuth2 认证账号的创建
/// - 账号信息的更新（名称、颜色、同步开关等）
///
/// 主要类型：
/// - AccountService: 账号服务实现
/// - AccountDto: 账号数据传输对象
/// - CreateAccountRequest: 创建账号请求
/// - UpdateAccountRequest: 更新账号请求
pub mod account_connection;
pub mod account_service;
pub mod attachment_service;

/// 邮件服务模块
///
/// 职责：
/// - 邮件列表查询（按文件夹、分类）
/// - 邮件详情获取
/// - 邮件全文搜索
/// - 邮件状态更新（已读/未读、星标）
/// - 邮件发送（支持密码和 OAuth2 认证）
/// - 邮件删除（软删除和硬删除）
/// - 邮件移动到其他文件夹
///
/// 主要类型：
/// - EmailService: 邮件服务实现
/// - EmailDto: 邮件数据传输对象
/// - EmailDetail: 邮件详情（包含正文）
/// - EmailCategory: 邮件分类枚举
/// - SendEmailRequest: 发送邮件请求
pub mod email_service;
pub mod mail_operation;
pub mod mail_send;

/// 标签服务模块
///
/// 职责：
/// - 自定义标签的创建、更新、删除
/// - 标签列表查询
/// - 邮件与标签的关联管理
///
/// 主要类型：
/// - LabelService: 标签服务实现
/// - LabelDto: 标签数据传输对象
/// - CreateLabelRequest: 创建标签请求
/// - UpdateLabelRequest: 更新标签请求
pub mod label_service;

/// 同步服务模块
///
/// 职责：
/// - 与邮件服务器（IMAP/SMTP）的同步
/// - 邮件同步（拉取新邮件、标记状态同步）
/// - 文件夹同步（文件夹列表更新）
/// - 同步状态的监控和报告
/// - 错误处理和重试机制
///
/// 主要类型：
/// - SyncService: 同步服务实现
pub mod sync_service;

// ───── 公共导出 ─────
// 以下导出的类型和结构体供 Command 层使用

/// 账号相关导出
///
/// - AccountDto: 账号信息展示
/// - AccountService: 账号服务实例
/// - CreateAccountRequest: 创建账号时的请求参数
/// - UpdateAccountRequest: 更新账号时的请求参数
pub use account_service::{AccountDto, AccountService, CreateAccountRequest, UpdateAccountRequest};

/// 附件相关导出
pub use attachment_service::{AttachmentDto, AttachmentService, InlineAttachmentDto};

/// 标签相关导出
///
/// - LabelDto: 标签信息展示
/// - LabelService: 标签服务实例
/// - CreateLabelRequest: 创建标签时的请求参数
/// - UpdateLabelRequest: 更新标签时的请求参数
pub use label_service::{CreateLabelRequest, LabelDto, LabelService, UpdateLabelRequest};

/// 同步相关导出
///
/// - SyncService: 同步服务实例
pub use sync_service::SyncService;

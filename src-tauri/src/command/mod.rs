///
/// Tauri 命令模块
///
/// 本模块定义了所有前端可以通过 Tauri invoke 调用的命令函数。
/// 每个命令都是一个异步函数，使用 #[tauri::command] 宏标记，
/// 可以从前端 JavaScript/TypeScript 代码中调用。
///
/// 架构说明：
/// - 这些命令函数充当应用层的控制器，接收前端请求并转发给服务层处理
/// - 命令函数不包含业务逻辑，只负责参数验证、日志记录和结果返回
/// - 使用 tauri::State 注入服务层依赖，实现依赖注入
///
/// 模块组织：
/// - account: 账号管理命令（创建、更新、删除、查询账号）
/// - auth: 认证授权命令（检测服务商、OAuth2 流程）
/// - email: 邮件操作命令（列表、搜索、标记、发送、删除等）
/// - label: 标签管理命令（创建、更新、删除标签，管理邮件标签）
/// - sync: 同步命令（触发同步、获取同步状态等）
///
/// 特性说明：
/// - 所有命令都使用 #[specta::specta] 标记，用于生成 TypeScript 类型定义
/// - 使用统一的错误类型 MailError 进行错误处理
/// - 命令函数都返回 Result<T, MailError>，便于前端统一处理错误
///
/// 调用方式：
/// 前端使用 invoke 调用命令，例如：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const accounts = await invoke('list_accounts');
/// ```
///

// ========== 子模块声明 ==========

///
/// 账号管理命令模块
///
/// 提供账号的完整 CRUD 功能：
/// - list_accounts: 列出所有账号
/// - get_account: 获取指定账号详情
/// - create_account: 创建新账号
/// - update_account: 更新账号信息
/// - delete_account: 删除账号
/// - update_account_password: 更新账号密码
///
pub mod account;

///
/// 认证授权命令模块
///
/// 提供邮箱服务提供商检测和 OAuth2 授权功能：
/// - detect_provider: 根据邮箱地址自动检测服务提供商
/// - list_providers: 列出所有支持的服务提供商
/// - start_oauth2: 启动 OAuth2 授权流程
/// - poll_oauth2: 轮询 OAuth2 授权状态
///
pub mod auth;

///
/// 邮件操作命令模块
///
/// 提供邮件的完整操作功能：
/// - list_emails: 列出邮件（支持分页）
/// - list_emails_by_category: 按分类列出邮件
/// - get_email: 获取邮件详情
/// - search_emails: 搜索邮件
/// - mark_as_read: 标记邮件为已读
/// - toggle_star: 切换邮件星标状态
/// - delete_emails: 删除邮件
/// - move_email_to_folder: 移动邮件到文件夹
/// - send_email: 发送邮件
///
pub mod email;

///
/// 标签管理命令模块
///
/// 提供邮件标签的管理功能：
/// - list_labels: 列出所有标签
/// - create_label: 创建新标签
/// - update_label: 更新标签
/// - delete_label: 删除标签
/// - add_label_to_email: 为邮件添加标签
/// - remove_label_from_email: 从邮件移除标签
/// - get_labels_for_email: 获取邮件的所有标签
/// - list_emails_by_label: 列出带有指定标签的邮件
///
pub mod label;

///
/// 同步命令模块
///
/// 提供邮件同步相关功能：
/// - sync_account: 同步指定账号
/// - get_folder_stats: 获取文件夹统计信息
///
pub mod sync;

// ========== 模块重新导出 ==========

///
/// 重新导出账号命令
///
/// 将 account 模块中的所有命令导出到当前模块作用域，
/// 这样在外部可以使用命令名直接引用，无需加模块前缀。
///
pub use account::*;

///
/// 重新导出认证命令
///
/// 将 auth 模块中的所有命令导出到当前模块作用域，
/// 便于在 lib.rs 中集中注册所有命令。
///
pub use auth::*;

///
/// 重新导出邮件命令
///
/// 将 email 模块中的所有命令导出到当前模块作用域。
///
pub use email::*;

///
/// 重新导出标签命令
///
/// 将 label 模块中的所有命令导出到当前模块作用域。
///
pub use label::*;

///
/// 重新导出同步命令
///
/// 将 sync 模块中的所有命令导出到当前模块作用域。
///
pub use sync::*;

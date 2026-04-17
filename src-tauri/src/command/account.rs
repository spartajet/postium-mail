///
/// 账号管理命令模块
///
/// 本模块提供用户邮箱账号的完整CRUD（创建、读取、更新、删除）功能。
/// 这些命令允许前端用户管理他们的邮件账号，包括添加新账号、
/// 查看账号列表、修改账号信息、删除账号以及更新账号密码。
///
/// 功能说明：
/// 1. 支持多种邮箱服务商（Gmail、Outlook、QQ邮箱等）
/// 2. 支持多种认证方式（OAuth2、用户名密码等）
/// 3. 账号信息安全存储在本地数据库中
/// 4. 提供账号状态管理（同步状态、连接状态等）
///
/// 账号生命周期：
/// 创建 → 配置（可选）→ 验证 → 同步 → 更新/删除
///
/// 数据流：
/// 前端调用命令 → AccountService → 数据库
///                           ↓
///                     AuthManager（认证管理）
///
/// 错误处理：
/// - 所有命令返回 Result<T, MailError>
/// - MailError 包含详细的错误信息，便于前端展示
///
/// 权限控制：
/// - 当前版本为单用户应用，所有账号都属于当前用户
/// - 未来版本可能添加多用户支持和权限控制
///
/// 依赖项：
/// - AccountService: 账号服务层，处理业务逻辑
/// - AccountDto: 账号数据传输对象
/// - CreateAccountRequest: 创建账号请求
/// - UpdateAccountRequest: 更新账号请求
///
// 导入必要的依赖项
use crate::error::MailError; // 自定义错误类型
use crate::service::account_service::AccountService; // 账号服务层
use crate::service::{AccountDto, CreateAccountRequest, UpdateAccountRequest}; // 数据传输对象

///
/// 列出所有账号
///
/// 获取当前用户的所有邮箱账号列表。此命令通常在应用启动时调用，
/// 用于在界面中显示用户的账号列表。
///
/// 功能说明：
/// 1. 从数据库查询所有账号记录
/// 2. 脱敏处理（不返回密码等敏感信息）
/// 3. 按创建时间或用户指定顺序排序
/// 4. 包含账号的同步状态和连接状态
///
/// 参数：
/// - service: AccountService实例，通过Tauri的State机制注入
///
/// 返回值：
/// - Ok(Vec<AccountDto>): 账号列表，每个元素包含：
///   - id: 账号唯一标识
///   - email: 邮箱地址
///   - provider: 服务商ID
///   - display_name: 显示名称
///   - is_active: 是否激活
///   - sync_status: 同步状态（idle/syncing/error）
///   - last_sync_time: 最后同步时间
/// - Err(MailError): 查询失败时的错误信息
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const accounts = await invoke('list_accounts');
/// accounts.forEach(account => {
///   console.log(`${account.display_name} (${account.email})`);
/// });
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn list_accounts(
    service: tauri::State<'_, AccountService>,
) -> Result<Vec<AccountDto>, MailError> {
    // 记录调试日志
    tracing::debug!("命令: 列出所有账号");

    // 调用服务层的list方法获取所有账号
    // 该方法会：
    // 1. 从数据库查询账号记录
    // 2. 过滤敏感信息（如密码）
    // 3. 返回账号DTO列表
    let accounts = service.list().await?;

    // 记录返回的账号数量
    tracing::debug!(count = accounts.len(), "返回 {} 个账号", accounts.len());

    // 返回账号列表
    Ok(accounts)
}

///
/// 获取指定账号详情
///
/// 根据账号ID获取单个账号的详细信息。
/// 此命令用于显示账号详情页面或编辑账号时的数据加载。
///
/// 功能说明：
/// 1. 根据ID查询账号
/// 2. 返回完整的账号信息（但不包含密码）
/// 3. 如果账号不存在，返回错误
///
/// 参数：
/// - service: AccountService实例
/// - id: 账号的唯一标识符
///
/// 返回值：
/// - Ok(AccountDto): 账号详细信息，包含：
///   - 所有list_accounts返回的字段
///   - 配置详情（IMAP/SMTP设置等）
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - DatabaseError: 数据库查询错误
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// try {
///   const account = await invoke('get_account', { id: 1 });
///   console.log('账号详情:', account);
/// } catch (error) {
///   console.error('账号不存在:', error);
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn get_account(
    service: tauri::State<'_, AccountService>,
    id: i32,
) -> Result<AccountDto, MailError> {
    // 记录调试日志，包含账号ID
    tracing::debug!(id, "命令: 获取账号");

    // 调用服务层获取账号详情
    // 该方法会：
    // 1. 根据ID查询数据库
    // 2. 验证账号是否存在
    // 3. 返回账号DTO
    service.get(id).await
}

///
/// 创建新账号
///
/// 添加一个新的邮箱账号到应用中。
/// 此命令是账号管理的核心功能，支持OAuth2和密码认证两种方式。
///
/// 功能说明：
/// 1. 验证邮箱地址格式
/// 2. 检测邮箱服务商（如果未指定）
/// 3. 验证认证信息（密码或OAuth2令牌）
/// 4. 连接IMAP/SMTP服务器验证配置
/// 5. 保存账号信息到数据库
/// 6. 初始化账号的文件夹结构
///
/// 参数：
/// - service: AccountService实例
/// - request: CreateAccountRequest对象，包含：
///   - email: 邮箱地址（必填）
///   - provider: 服务商ID（可选，可自动检测）
///   - display_name: 显示名称（可选，默认使用邮箱前缀）
///   - password: 密码（OAuth2账号可不填）
///   - oauth2_state: OAuth2状态（使用OAuth2时填入）
///   - imap_config: 自定义IMAP配置（可选）
///   - smtp_config: 自定义SMTP配置（可选）
///
/// 返回值：
/// - Ok(AccountDto): 创建成功的账号信息
/// - Err(MailError):
///   - ValidationError: 邮箱格式错误或验证失败
///   - AuthError: 认证失败
///   - ConnectionError: 无法连接到邮件服务器
///   - DatabaseError: 数据库保存失败
///
/// 使用场景：
/// 1. 用户手动添加账号（输入邮箱和密码）
/// 2. 通过OAuth2添加账号（调用start_oauth2后自动创建）
/// 3. 导入账号配置
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const account = await invoke('create_account', {
///   request: {
///     email: 'user@example.com',
///     provider: 'example-provider',
///     display_name: '我的邮箱',
///     password: 'my-password'
///   }
/// });
/// console.log('账号创建成功，ID:', account.id);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn create_account(
    service: tauri::State<'_, AccountService>,
    request: CreateAccountRequest,
) -> Result<AccountDto, MailError> {
    // 记录信息日志，包含邮箱和服务商
    tracing::info!(email = %request.email, provider = %request.provider, "命令: 创建账号");

    // 调用服务层创建账号
    // 该方法会执行完整的创建流程：
    // 1. 参数验证
    // 2. 服务商检测（如果需要）
    // 3. 认证信息验证
    // 4. 服务器连接测试
    // 5. 保存到数据库
    // 6. 初始化同步状态
    let result = service.create(request).await?;

    // 记录创建成功的日志
    tracing::info!(id = result.id, email = %result.email, "账号创建成功");

    // 返回创建的账号信息
    Ok(result)
}

///
/// 更新账号信息
///
/// 修改现有账号的配置信息。
/// 此命令用于更新账号的显示名称、服务器配置等非认证信息。
///
/// 注意事项：
/// - 此命令不更新密码（使用update_account_password）
/// - 更新OAuth2账号的认证信息需要重新授权
/// - 修改服务器配置后会自动验证连接
///
/// 功能说明：
/// 1. 验证账号存在
/// 2. 更新账号的可修改字段
/// 3. 如果修改了服务器配置，验证新配置
/// 4. 保存更新到数据库
/// 5. 触发同步服务重新加载配置
///
/// 参数：
/// - service: AccountService实例
/// - request: UpdateAccountRequest对象，包含：
///   - id: 账号ID（必填）
///   - display_name: 显示名称（可选）
///   - is_active: 是否激活（可选）
///   - imap_config: IMAP服务器配置（可选）
///   - smtp_config: SMTP服务器配置（可选）
///   - sync_enabled: 是否启用同步（可选）
///   - sync_interval: 同步间隔（可选）
///
/// 返回值：
/// - Ok(AccountDto): 更新后的账号信息
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - ValidationError: 配置无效
///   - ConnectionError: 新配置无法连接
///   - DatabaseError: 数据库更新失败
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const updated = await invoke('update_account', {
///   request: {
///     id: 1,
///     display_name: '新的显示名称',
///     sync_interval: 300  // 每5分钟同步一次
///   }
/// });
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn update_account(
    service: tauri::State<'_, AccountService>,
    request: UpdateAccountRequest,
) -> Result<AccountDto, MailError> {
    // 记录信息日志
    tracing::info!(id = request.id, "命令: 更新账号");

    // 调用服务层更新账号
    // 该方法会：
    // 1. 验证账号存在
    // 2. 应用更新字段
    // 3. 验证配置（如果修改了服务器设置）
    // 4. 保存到数据库
    // 5. 通知相关服务更新状态
    service.update(request).await
}

///
/// 删除账号
///
/// 从应用中永久删除指定的邮箱账号及其所有相关数据。
///
/// ⚠️ 警告：此操作不可逆！
/// 删除账号时会同时删除：
/// - 账号配置信息
/// - 所有同步的邮件数据
/// - 邮件文件夹结构
/// - 标签和分类
/// - 同步历史记录
///
/// 功能说明：
/// 1. 验证账号存在
/// 2. 停止该账号的同步任务
/// 3. 清理数据库中的相关数据
/// 4. 清理认证信息
/// 5. 删除账号记录
///
/// 参数：
/// - service: AccountService实例
/// - id: 要删除的账号ID
///
/// 返回值：
/// - Ok(()): 删除成功
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - DatabaseError: 数据库删除失败
///   - SyncError: 无法停止同步任务
///
/// 安全提示：
/// - 建议在前端添加确认对话框
/// - 可以添加"软删除"功能（标记为已删除而非立即删除）
/// - 考虑添加"导出数据"选项，让用户删除前备份数据
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// import { confirm } from '@tauri-apps/api/dialog';
///
/// const confirmed = await confirm('确定要删除此账号吗？此操作不可撤销！');
/// if (confirmed) {
///   await invoke('delete_account', { id: 1 });
///   console.log('账号已删除');
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn delete_account(
    service: tauri::State<'_, AccountService>,
    id: i32,
) -> Result<(), MailError> {
    // 记录信息日志
    tracing::info!(id, "命令: 删除账号");

    // 调用服务层删除账号
    // 该方法会：
    // 1. 验证账号存在
    // 2. 停止正在进行的同步
    // 3. 删除所有相关数据
    // 4. 清理认证凭证
    // 5. 从数据库删除记录
    service.delete(id).await
}

///
/// 更新账号密码
///
/// 修改账号的登录密码。
/// 此命令用于账号密码过期或用户主动修改密码的场景。
///
/// 功能说明：
/// 1. 验证账号存在
/// 2. 验证新密码格式
/// 3. 使用新密码连接服务器验证
/// 4. 更新数据库中的密码（加密存储）
/// 5. 更新认证管理器中的凭证
/// 6. 重启同步任务（如果正在同步）
///
/// 参数：
/// - service: AccountService实例
/// - id: 账号ID
/// - password: 新密码（明文，会在服务端加密）
///
/// 返回值：
/// - Ok(()): 密码更新成功
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - ValidationError: 密码格式无效
///   - AuthError: 新密码无法登录
///   - DatabaseError: 数据库更新失败
///
/// 安全说明：
/// - 密码在传输过程中使用加密通道（Tauri IPC）
/// - 密码在数据库中加密存储
/// - 不会记录密码到日志
/// - OAuth2账号无法使用此命令（需要重新授权）
///
/// 使用场景：
/// 1. 用户定期更换密码
/// 2. 密码被重置后更新
/// 3. 密码输入错误后更正
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// import { open } from '@tauri-apps/api/dialog';
///
/// // 假设有一个输入框让用户输入新密码
/// const newPassword = await open({
///   title: '输入新密码',
///   multiple: false
/// });
///
/// if (newPassword) {
///   await invoke('update_account_password', {
///     id: 1,
///     password: newPassword
///   });
///   console.log('密码更新成功');
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn update_account_password(
    service: tauri::State<'_, AccountService>,
    id: i32,
    password: String,
) -> Result<(), MailError> {
    // 记录信息日志（注意：不记录密码本身）
    tracing::info!(id, "命令: 更新账号密码");

    // 调用服务层更新密码
    // 该方法会：
    // 1. 验证账号存在
    // 2. 验证新密码强度（可选）
    // 3. 使用新密码测试登录
    // 4. 加密新密码
    // 5. 更新数据库
    // 6. 更新认证管理器
    // 7. 重启同步
    service.update_password(id, password).await
}

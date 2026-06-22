use crate::domain::providers::ProviderInfo;
/// 认证授权命令模块
///
/// 本模块提供与邮箱服务商检测和OAuth2认证相关的Tauri命令。
/// 这些命令允许前端用户：
/// 1. 根据邮箱地址自动检测邮箱服务商（如Gmail、Outlook等）
/// 2. 查看所有支持的邮箱服务商列表
/// 3. 启动OAuth2授权流程
/// 4. 轮询OAuth2授权状态
///
/// 工作流程说明：
/// 1. 用户输入邮箱地址 → detect_provider 检测服务商
/// 2. 如果服务商支持OAuth2 → start_oauth2 启动授权流程
/// 3. 用户在浏览器中完成授权 → poll_oauth2 轮询授权结果
/// 4. 授权成功后自动创建账号
///
/// 安全特性：
/// - OAuth2使用PKCE（Proof Key for Code Exchange）增强安全性
/// - 授权码通过本地HTTP回调获取，避免令牌泄露
/// - 访问令牌安全存储在本地数据库中
///
/// 依赖项：
/// - ProviderPool: 提供所有邮箱服务商的配置信息
/// - OAuth2Manager: 管理OAuth2授权流程
///
use crate::domain::providers::detect::ProviderDetectionResult;
use crate::domain::providers::pool::ProviderPool;
use crate::error::MailError;
use crate::infrastructure::auth::oauth2::{OAuth2AuthUrl, OAuth2PollResult};
use std::sync::Arc;

///
/// 检测邮箱服务商
///
/// 根据用户提供的邮箱地址，自动识别并返回对应的邮箱服务商信息。
/// 此命令在用户添加账号时调用，用于简化配置流程。
///
/// 功能说明：
/// 1. 解析邮箱地址的域名部分
/// 2. 在ProviderPool中查找匹配的服务商配置
/// 3. 返回服务商ID、支持的协议、服务器配置等信息
///
/// 支持的服务商示例：
/// - Gmail (gmail.com)
/// - Outlook (outlook.com, hotmail.com)
/// - QQ邮箱 (qq.com)
/// - 163邮箱 (163.com)
/// - 126邮箱 (126.com)
/// 等等
///
/// 参数：
/// - email: 用户的邮箱地址（例如: user@example.com）
/// - pool: ProviderPool实例，包含所有服务商的配置信息
///
/// 返回值：
/// - Ok(ProviderDetectionResult): 检测到的服务商信息，包含：
///   - provider_id: 服务商唯一标识
///   - provider_name: 服务商名称
///   - imap_config: IMAP服务器配置（可选）
///   - smtp_config: SMTP服务器配置（可选）
///   - supports_oauth2: 是否支持OAuth2认证
/// - Err(MailError): 检测失败时的错误信息
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const result = await invoke('detect_provider', { email: 'user@gmail.com' });
/// console.log(result.provider_id); // 输出: 'gmail'
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn detect_provider(
    email: String,
    pool: tauri::State<'_, Arc<ProviderPool>>,
) -> Result<ProviderDetectionResult, MailError> {
    // 记录调试日志，便于追踪检测过程
    tracing::debug!(email = %email, "检测邮箱服务商");

    // 调用领域层的detect_provider函数进行实际检测
    // 该函数会解析邮箱域名并在ProviderPool中查找匹配项
    let result = crate::domain::providers::detect::detect_provider(&email, &pool);

    // 记录检测结果
    tracing::info!(email = %email, provider = ?result.provider_id, "检测到服务商");

    Ok(result)
}

///
/// 列出所有邮箱服务商
///
/// 返回应用程序支持的所有邮箱服务商的完整列表。
/// 此命令通常用于：
/// 1. 在用户界面中显示服务商选择器
/// 2. 让用户查看支持的服务商列表
/// 3. 作为调试工具验证服务商配置
///
/// 功能说明：
/// 1. 从ProviderPool中获取所有已配置的服务商
/// 2. 返回每个服务商的基本信息和能力
///
/// 参数：
/// - 无
///
/// 返回值：
/// - Ok(Vec<ProviderInfo>): 服务商列表，每个元素包含：
///   - id: 服务商唯一标识
///   - name: 服务商显示名称
///   - icon: 服务商图标URL
///   - domains: 支持的邮箱域名列表
///   - supports_oauth2: 是否支持OAuth2
/// - Err(MailError): 获取失败时的错误信息
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const providers = await invoke('list_providers');
/// providers.forEach(p => console.log(p.name));
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn list_providers() -> Result<Vec<ProviderInfo>, MailError> {
    // 记录调试日志
    tracing::debug!("列出所有服务商");

    // 调用领域层函数获取服务商列表
    crate::domain::providers::detect::list_providers()
}

///
/// 启动OAuth2授权流程
///
/// 为指定的邮箱账号启动OAuth2授权流程。此命令会：
/// 1. 生成授权URL和回调端点
/// 2. 在本地启动HTTP服务器监听回调
/// 3. 返回授权URL供前端打开浏览器
///
/// 工作流程：
/// 1. 生成随机的state参数，用于防止CSRF攻击
/// 2. 启动本地HTTP服务器（默认监听随机端口）
/// 3. 构建OAuth2授权URL，包含必要的参数
/// 4. 返回URL和本地回调地址
/// 5. 前端打开授权URL，用户在浏览器中完成授权
/// 6. 服务提供商重定向到本地回调地址
/// 7. 本地服务器接收授权码
///
/// 参数：
/// - manager: OAuth2Manager实例，管理OAuth2流程
/// - provider_id: 服务商ID（如'gmail'、'outlook'）
/// - email: 用户的邮箱地址
/// - display_name: 可选，账号显示名称（用于UI展示）
///
/// 返回值：
/// - Ok(OAuth2AuthUrl): 授权URL信息，包含：
///   - url: 授权URL，需要在浏览器中打开
///   - port: 本地回调服务器端口号
///   - state: 用于后续轮询的状态标识
/// - Err(MailError): 启动失败时的错误信息
///
/// 安全说明：
/// - 使用PKCE增强安全性
/// - state参数用于防止CSRF攻击
/// - 授权码通过本地回调获取，不经过第三方
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const { url, state } = await invoke('start_oauth2', {
///   provider_id: 'gmail',
///   email: 'user@gmail.com',
///   display_name: '我的Gmail'
/// });
/// // 打开浏览器进行授权
/// window.open(url, '_blank');
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn start_oauth2(
    manager: tauri::State<'_, Arc<crate::infrastructure::auth::oauth2::OAuth2Manager>>,
    provider_id: String,
    email: String,
    display_name: Option<String>,
) -> Result<OAuth2AuthUrl, MailError> {
    // 记录信息日志
    tracing::info!(provider_id = %provider_id, email = %email, "命令: 启动 OAuth2");

    // 调用OAuth2Manager启动授权流程
    // 该方法会：
    // 1. 生成state和code_verifier
    // 2. 启动本地HTTP服务器
    // 3. 构建授权URL
    // 4. 保存授权状态供后续轮询使用
    let result = manager
        .start_auth(&provider_id, &email, display_name.as_deref())
        .await?;

    // 记录授权URL已生成
    tracing::info!(provider_id = %provider_id, port = result.port, "OAuth2 授权 URL 已生成");

    Ok(result)
}

///
/// 轮询OAuth2授权状态
///
/// 定期查询OAuth2授权流程的状态，检查用户是否已完成授权。
/// 前端需要调用此命令来获取授权结果。
///
/// 工作流程：
/// 1. 前端定时调用此命令（例如每秒一次）
/// 2. 后端检查本地HTTP服务器是否接收到回调
/// 3. 如果收到授权码，交换访问令牌
/// 4. 如果授权成功，自动创建账号并保存凭证
/// 5. 返回授权状态给前端
///
/// 状态说明：
/// - Pending: 等待用户完成授权
/// - Success: 授权成功，账号已创建
/// - Failed: 授权失败或被取消
/// - Expired: 授权流程超时
///
/// 参数：
/// - manager: OAuth2Manager实例，管理OAuth2流程
/// - state: 启动OAuth2时返回的状态标识
///
/// 返回值：
/// - Ok(OAuth2PollResult): 轮询结果，包含：
///   - status: 授权状态（pending/success/failed/expired）
///   - account_id: 授权成功时的账号ID（仅success状态）
///   - error: 失败时的错误信息（仅failed状态）
/// - Err(MailError): 轮询过程出错
///
/// 使用建议：
/// - 使用指数退避策略，避免过于频繁的轮询
/// - 收到success或failed状态后停止轮询
/// - 设置超时时间（如5分钟），超时后停止轮询
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// async function pollOAuth2State(state: string) {
///   const result = await invoke('poll_oauth2', { state });
///   if (result.status === 'success') {
///     console.log('授权成功，账号ID:', result.account_id);
///     // 跳转到账号列表
///   } else if (result.status === 'failed') {
///     console.error('授权失败:', result.error);
///   } else if (result.status === 'pending') {
///     // 继续轮询
///     setTimeout(() => pollOAuth2State(state), 1000);
///   }
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn poll_oauth2(
    manager: tauri::State<'_, Arc<crate::infrastructure::auth::oauth2::OAuth2Manager>>,
    state: String,
) -> Result<OAuth2PollResult, MailError> {
    // 记录追踪日志，state值可能包含敏感信息但在这里用于调试
    tracing::trace!(state = %state, "轮询 OAuth2 状态");

    // 调用OAuth2Manager检查授权状态
    // 该方法会：
    // 1. 检查本地HTTP服务器是否收到回调
    // 2. 如果收到授权码，使用它交换访问令牌
    // 3. 保存令牌到数据库
    // 4. 创建或更新账号记录
    // 5. 返回当前状态
    manager.poll_oauth2(&state).await
}

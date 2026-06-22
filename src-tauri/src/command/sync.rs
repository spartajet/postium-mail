///
/// 邮件同步命令模块
///
/// 本模块提供与邮件同步相关的 Tauri 命令。
/// 邮件同步是邮件客户端的核心功能，负责从邮件服务器获取最新的邮件，
/// 并将本地修改（如已发送邮件、删除操作）同步到服务器。
///
/// 主要功能：
/// 1. 手动同步：
///    - sync_account: 手动触发指定账号的同步操作
///
/// 2. 同步状态查询：
///    - get_folder_stats: 获取账号各文件夹的统计信息
///
/// 同步机制说明：
/// - 支持增量同步：只同步新增或修改的邮件，提高效率
/// - 支持双向同步：既从服务器拉取邮件，也推送本地修改
/// - 支持多账号并行同步：不同账号可以同时同步
/// - 提供同步进度通知：通过事件向前端发送同步进度
///
/// 同步类型：
/// 1. IMAP 同步：
///    - 拉取新邮件
///    - 同步邮件状态（已读、删除、移动等）
///    - 同步文件夹结构
///
/// 2. SMTP 同步：
///    - 推送已发送邮件到服务器
///    - 推送草稿（如果支持）
///
/// 架构说明：
/// - 这些命令充当应用层的控制器，接收前端请求并转发给 SyncService
/// - 命令函数不包含业务逻辑，只负责参数验证、日志记录和结果返回
/// - 使用 tauri::State 注入 SyncService 实例，实现依赖注入
///
/// 数据流：
/// 前端 invoke 调用 → 命令函数 → SyncService → IMAP/SMTP 协议层
///                                ↓
///                            发送进度事件 → 前端 UI
///
/// 错误处理：
/// - 所有命令返回 Result<T, MailError>
/// - MailError 包含详细的错误信息，便于前端展示给用户
/// - 同步过程中的错误会记录到日志
///
/// 安全说明：
/// - 使用 TLS/SSL 加密连接邮件服务器
/// - 认证凭证安全存储在本地数据库
/// - 同步过程中不会泄露邮件内容到外部服务器
///
/// 依赖项：
/// - SyncService: 同步服务层，处理同步的业务逻辑
/// - FolderStat: 文件夹统计信息
///
use crate::domain::sync::FolderStat;
use crate::error::MailError;

///
/// 手动同步账号
///
/// 手动触发指定账号的邮件同步操作。
/// 此命令用于用户主动刷新邮件的场景，与自动同步（后台定时任务）不同。
///
/// 功能说明：
/// 1. 从邮件服务器获取最新的邮件列表
/// 2. 下载新邮件的完整内容（正文、附件等）
/// 3. 同步邮件状态（已读、删除、移动等）
/// 4. 推送本地修改到服务器（如已发送邮件）
/// 5. 更新文件夹结构
/// 6. 实时向前端发送同步进度事件
///
/// 同步过程：
/// 1. 连接到邮件服务器（IMAP/SMTP）
/// 2. 获取服务器端的邮件列表（使用 UID/IMAP ID）
/// 3. 比较本地和远程邮件，确定需要同步的内容
/// 4. 下载新邮件或更新已有邮件
/// 5. 上传本地修改（如标记已读、删除等操作）
/// 6. 更新本地数据库
/// 7. 清理已删除的邮件
///
/// 同步进度事件：
/// 同步过程中会通过 Tauri 事件向前端发送进度通知：
/// - SyncProgressEvent {
///     account_id: i32,
///     stage: String,           // 当前阶段（如"连接中"、"下载中"、"上传中"等）
///     progress: f64,           // 进度百分比（0.0 - 1.0）
///     current: usize,          // 当前进度值
///     total: usize,            // 总数量
///   }
///
/// 参数：
/// - service: SyncService 实例，通过 Tauri 的 State 机制注入
/// - app_handle: Tauri 应用句柄，用于发送进度事件
/// - account_id: 要同步的账号 ID
///
/// 返回值：
/// - Ok(()): 同步成功
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - AuthError: 认证失败（密码错误、令牌过期等）
///   - ConnectionError: 无法连接到邮件服务器
///   - SyncError: 同步过程中出错
///   - TimeoutError: 同步超时
///
/// 同步策略：
/// - 增量同步：只同步新增或修改的邮件
/// - 全量同步：首次同步或长时间未同步时进行全量同步
/// - 错误重试：遇到网络错误时自动重试（最多 3 次）
///
/// 性能优化：
/// - 使用 IDLE 命令（如果支持）实时接收新邮件通知
/// - 使用管道（pipelining）提高同步速度
/// - 附件按需下载：首次只下载文本，附件在查看时下载
///
/// 使用场景：
/// 1. 用户点击"刷新"按钮时
/// 2. 用户重新连接网络后
/// 3. 用户添加新账号后首次同步
/// 4. 用户账号密码修改后
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// import { listen } from '@tauri-apps/api/event';
///
/// // 监听同步进度事件
/// const unlisten = await listen('sync-progress', (event) => {
///   const progress = event.payload as SyncProgressEvent;
///   console.log(`同步进度: ${progress.progress * 100}%`);
///   console.log(`当前阶段: ${progress.stage}`);
/// });
///
/// // 开始同步
/// try {
///   await invoke('sync_account', { account_id: 1 });
///   console.log('同步完成');
/// } catch (error) {
///   console.error('同步失败:', error);
/// } finally {
///   // 取消监听
///   unlisten();
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn sync_account(
    service: tauri::State<'_, crate::service::SyncService>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), MailError> {
    // 记录信息日志
    tracing::info!(account_id, "命令: 手动同步账号");

    // 调用服务层执行同步操作
    // 该方法会：
    // 1. 验证账号存在
    // 2. 获取账号的认证信息
    // 3. 连接到邮件服务器
    // 4. 执行增量同步或全量同步
    // 5. 通过 app_handle 发送进度事件
    // 6. 更新本地数据库
    let result = service
        .sync_account_with_progress(app_handle, account_id)
        .await;

    // 根据同步结果记录相应的日志
    match &result {
        Ok(()) => {
            tracing::info!(account_id, "手动同步完成");
        }
        Err(e) => {
            tracing::error!(
                account_id,
                error = %e,
                "手动同步失败"
            );
        }
    }

    // 返回同步结果
    result
}

///
/// 获取文件夹统计信息
///
/// 获取指定账号各个文件夹的统计信息，包括邮件数量、未读数量等。
/// 此命令用于在侧边栏显示文件夹的统计信息（如"收件箱 (5)"）。
///
/// 功能说明：
/// 1. 从数据库查询各文件夹的邮件数量
/// 2. 计算未读邮件数量
/// 3. 返回文件夹统计信息
///
/// 返回的统计信息：
/// - folder_name: 文件夹名称（如 "INBOX"、"Sent" 等）
/// - total_emails: 邮件总数
/// - unread_count: 未读邮件数量
/// - last_sync_time: 最后同步时间（可选）
///
/// 常见文件夹：
/// - INBOX: 收件箱
/// - Sent: 已发送
/// - Drafts: 草稿箱
/// - Trash: 垃圾箱
/// - Spam: 垃圾邮件
/// - Archive: 归档
/// - 用户自定义的文件夹
///
/// 参数：
/// - service: SyncService 实例
/// - account_id: 账号 ID
///
/// 返回值：
/// - Ok(Vec<FolderStat>): 文件夹统计信息列表，按文件夹名称排序
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - DatabaseError: 数据库查询错误
///
/// 使用场景：
/// 1. 应用启动时加载文件夹统计
/// 2. 同步完成后更新统计信息
/// 3. 用户切换账号时更新统计
/// 4. 定期刷新统计信息（如每分钟）
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// const stats = await invoke('get_folder_stats', { account_id: 1 });
/// stats.forEach(stat => {
///   console.log(`${stat.folder_name}: ${stat.total_emails} 封邮件, ${stat.unread_count} 封未读`);
/// });
/// // 输出示例:
/// // INBOX: 100 封邮件, 5 封未读
/// // Sent: 50 封邮件, 0 封未读
/// // Drafts: 3 封邮件, 0 封未读
/// // Trash: 10 封邮件, 0 封未读
/// ```
///
/// 扩展功能（可选）：
/// - 支持缓存统计信息，减少数据库查询
/// - 支持实时更新（通过事件通知）
/// - 支持自定义文件夹图标
/// - 支持文件夹颜色配置
///
#[tauri::command]
#[specta::specta]
pub async fn get_folder_stats(
    service: tauri::State<'_, crate::service::SyncService>,
    account_id: i32,
) -> Result<Vec<FolderStat>, MailError> {
    // 记录调试日志
    tracing::debug!(account_id, "命令: 获取文件夹统计");

    // 调用服务层获取文件夹统计
    // 该方法会：
    // 1. 验证账号存在
    // 2. 查询各文件夹的邮件数量
    // 3. 计算未读数量
    // 4. 返回统计信息列表
    service.get_folder_stats(account_id).await
}

///
/// 标签管理命令模块
///
/// 本模块提供与邮件标签（Label）管理相关的 Tauri 命令。
/// 标签是邮件组织和分类的重要工具，允许用户为邮件添加自定义标签，
/// 以便更好地组织和管理邮件。
///
/// 主要功能分类：
/// 1. 标签 CRUD 操作：
///    - list_labels: 列出账号的所有标签
///    - create_label: 创建新标签
///    - update_label: 更新标签信息
///    - delete_label: 删除标签
///
/// 2. 邮件标签关联：
///    - add_label_to_email: 为邮件添加标签
///    - remove_label_from_email: 从邮件移除标签
///    - get_labels_for_email: 获取邮件的所有标签
///    - list_emails_by_label: 列出带有指定标签的所有邮件
///
/// 标签特性：
/// - 一个邮件可以有多个标签
/// - 标签具有颜色和图标，便于视觉识别
/// - 标签是账号级别的，不同账号的标签相互独立
/// - 支持自定义标签名称、颜色和图标
///
/// 架构说明：
/// - 这些命令充当应用层的控制器，接收前端请求并转发给 LabelService
/// - 命令函数不包含业务逻辑，只负责参数验证、日志记录和结果返回
/// - 使用 tauri::State 注入 LabelService 实例，实现依赖注入
///
/// 数据流：
/// 前端 invoke 调用 → 命令函数 → LabelService → 数据库
///
/// 使用场景：
/// - 邮件分类：通过标签将邮件分类（如"工作"、"个人"、"重要"）
/// - 邮件筛选：快速查看具有特定标签的邮件
/// - 邮件优先级：使用标签标记重要邮件
/// - 邮件归档：按标签归档邮件
///
/// 错误处理：
/// - 所有命令返回 Result<T, MailError>
/// - MailError 包含详细的错误信息，便于前端展示给用户
///
/// 安全说明：
/// - 标签操作不会影响邮件的实际内容
/// - 删除标签只会移除标签与邮件的关联，不会删除邮件
///
/// 依赖项：
/// - LabelService: 标签服务层，处理标签的业务逻辑
/// - LabelDto: 标签数据传输对象
/// - CreateLabelRequest: 创建标签请求
/// - UpdateLabelRequest: 更新标签请求
///
use crate::error::MailError;
use crate::service::label_service::{
    CreateLabelRequest, LabelDto, LabelService, UpdateLabelRequest,
};

///
/// 列出所有标签
///
/// 获取指定账号的所有标签列表。
/// 此命令通常在应用启动时或标签管理界面加载时调用。
///
/// 功能说明：
/// 1. 从数据库查询指定账号的所有标签
/// 2. 返回标签的基本信息（ID、名称、颜色、图标等）
/// 3. 包含每个标签关联的邮件数量（可选，由服务层决定）
/// 4. 按创建时间或名称排序（由服务层决定）
///
/// 参数：
/// - service: LabelService 实例，通过 Tauri 的 State 机制注入
/// - account_id: 账号 ID，指定要查询哪个账号的标签
///
/// 返回值：
/// - Ok(Vec<LabelDto>): 标签列表，每个元素包含：
///   - id: 标签唯一标识
///   - account_id: 所属账号 ID
///   - name: 标签名称
///   - color: 标签颜色（十六进制颜色码，如 "#FF5733"）
///   - icon: 标签图标（可选，如 "tag"、"star" 等）
///   - email_count: 关联的邮件数量（可选）
///   - created_at: 创建时间
///   - updated_at: 更新时间
/// - Err(MailError):
///   - NotFound: 账号不存在
///   - DatabaseError: 数据库查询错误
///
/// 使用场景：
/// 1. 用户打开标签管理界面时
/// 2. 在邮件列表中显示标签选项时
/// 3. 应用启动时加载标签列表
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const labels = await invoke('list_labels', { account_id: 1 });
/// labels.forEach(label => {
///   console.log(`${label.name} (${label.email_count} 封邮件)`);
/// });
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn list_labels(
    service: tauri::State<'_, LabelService>,
    account_id: i32,
) -> Result<Vec<LabelDto>, MailError> {
    // 记录调试日志
    tracing::debug!(account_id, "命令: 列出标签");

    // 调用服务层获取标签列表
    // 该方法会：
    // 1. 验证账号存在
    // 2. 从数据库查询标签
    // 3. 返回标签列表
    service.list_labels(account_id).await
}

///
/// 创建新标签
///
/// 为指定账号创建一个新的标签。
/// 此命令允许用户自定义标签，以便更好地组织邮件。
///
/// 功能说明：
/// 1. 验证标签名称的唯一性（同一账号下标签名称不能重复）
/// 2. 验证颜色格式（必须是有效的十六进制颜色码）
/// 3. 保存标签到数据库
/// 4. 返回创建的标签信息
///
/// 参数：
/// - service: LabelService 实例
/// - request: CreateLabelRequest 对象，包含：
///   - account_id: 所属账号 ID（必填）
///   - name: 标签名称（必填，不能为空）
///   - color: 标签颜色（可选，默认使用系统生成的颜色）
///   - icon: 标签图标（可选，默认不设置图标）
///
/// 返回值：
/// - Ok(LabelDto): 创建成功的标签信息，包含：
///   - id: 新创建的标签 ID
///   - account_id: 所属账号 ID
///   - name: 标签名称
///   - color: 标签颜色
///   - icon: 标签图标
///   - created_at: 创建时间
///   - updated_at: 更新时间
/// - Err(MailError):
///   - ValidationError: 标签名称无效或颜色格式错误
///   - ConflictError: 标签名称已存在
///   - DatabaseError: 数据库保存失败
///
/// 颜色格式说明：
/// - 必须是十六进制颜色码格式，如 "#FF5733"
/// - 支持简写格式，如 "#F53"（会被转换为 "#FF5533"）
/// - 如果不指定颜色，系统会自动生成一个随机颜色
///
/// 图标建议：
/// - 使用常见的图标名称，如 "tag"、"star"、"flag"、"bookmark" 等
/// - 图标应由前端根据名称渲染
/// - 如果不指定图标，使用默认的标签图标
///
/// 使用场景：
/// 1. 用户在标签管理界面点击"创建标签"时
/// 2. 用户为邮件创建新标签时
/// 3. 导入标签配置时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// const label = await invoke('create_label', {
///   request: {
///     account_id: 1,
///     name: '工作',
///     color: '#3498db',
///     icon: 'briefcase'
///   }
/// });
/// console.log('标签创建成功，ID:', label.id);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn create_label(
    service: tauri::State<'_, LabelService>,
    request: CreateLabelRequest,
) -> Result<LabelDto, MailError> {
    // 记录信息日志
    tracing::info!(
        account_id = request.account_id,
        name = %request.name,
        "命令: 创建标签"
    );

    // 调用服务层创建标签
    // 该方法会：
    // 1. 验证标签名称和颜色
    // 2. 检查名称是否重复
    // 3. 保存到数据库
    // 4. 返回创建的标签
    let result = service.create_label(request).await?;

    // 记录创建成功
    tracing::info!(
        id = result.id,
        name = %result.name,
        "标签创建成功"
    );

    Ok(result)
}

///
/// 更新标签信息
///
/// 修改现有标签的名称、颜色或图标。
/// 此命令用于用户需要修改标签配置的场景。
///
/// 功能说明：
/// 1. 验证标签存在
/// 2. 如果修改了名称，检查新名称是否与现有标签冲突
/// 3. 如果修改了颜色，验证颜色格式
/// 4. 更新数据库中的标签信息
/// 5. 返回更新后的标签信息
///
/// 参数：
/// - service: LabelService 实例
/// - id: 标签 ID（必填）
/// - request: UpdateLabelRequest 对象，包含：
///   - name: 新的标签名称（可选）
///   - color: 新的标签颜色（可选）
///   - icon: 新的标签图标（可选）
///
/// 注意事项：
/// - 至少需要提供 name、color 或 icon 中的一个
/// - 如果所有字段都为 None，返回错误
/// - 名称修改会影响该标签在所有邮件上的显示
/// - 颜色和图标修改会在界面上即时生效
///
/// 返回值：
/// - Ok(LabelDto): 更新后的标签信息
/// - Err(MailError):
///   - NotFound: 标签不存在
///   - ValidationError: 参数无效
///   - ConflictError: 新名称与现有标签冲突
///   - DatabaseError: 数据库更新错误
///
/// 使用场景：
/// 1. 用户编辑标签属性时
/// 2. 用户修改标签颜色以更好地区分标签时
/// 3. 用户更改标签名称时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// const updated = await invoke('update_label', {
///   id: 1,
///   request: {
///     name: '重要工作',
///     color: '#e74c3c',
///     icon: 'star'
///   }
/// });
/// console.log('标签更新成功:', updated.name);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn update_label(
    service: tauri::State<'_, LabelService>,
    id: i32,
    request: UpdateLabelRequest,
) -> Result<LabelDto, MailError> {
    // 记录信息日志
    tracing::info!(id, "命令: 更新标签");

    // 调用服务层更新标签
    // 该方法会：
    // 1. 验证标签存在
    // 2. 验证更新参数
    // 3. 检查名称冲突
    // 4. 更新数据库
    // 5. 返回更新后的标签
    service.update_label(id, request).await
}

///
/// 删除标签
///
/// 从账号中永久删除指定的标签。
///
/// ⚠️ 警告：删除标签会同时移除所有邮件与该标签的关联！
/// 删除操作不可恢复。
///
/// 功能说明：
/// 1. 验证标签存在
/// 2. 移除标签与所有邮件的关联
/// 3. 从数据库中删除标签记录
/// 4. 更新标签计数
///
/// 删除行为：
/// - 只删除标签本身，不会删除任何邮件
/// - 所有带有该标签的邮件都会失去该标签
/// - 如果邮件没有其他标签，仍会保留在原文件夹中
///
/// 参数：
/// - service: LabelService 实例
/// - id: 要删除的标签 ID
///
/// 返回值：
/// - Ok(()): 删除成功
/// - Err(MailError):
///   - NotFound: 标签不存在
///   - DatabaseError: 数据库删除失败
///
/// 安全提示：
/// - 建议在前端添加确认对话框
/// - 可以显示该标签关联的邮件数量
/// - 可以提供"合并标签"功能，将当前标签的邮件合并到其他标签
///
/// 使用场景：
/// 1. 用户在标签管理界面删除标签时
/// 2. 用户清理不再需要的标签时
/// 3. 系统清理废弃标签时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// import { confirm } from '@tauri-apps/api/dialog';
///
/// const confirmed = await confirm(
///   '确定要删除此标签吗？此操作不可撤销！'
/// );
/// if (confirmed) {
///   await invoke('delete_label', { id: 1 });
///   console.log('标签已删除');
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn delete_label(
    service: tauri::State<'_, LabelService>,
    id: i32,
) -> Result<(), MailError> {
    // 记录信息日志
    tracing::info!(id, "命令: 删除标签");

    // 调用服务层删除标签
    // 该方法会：
    // 1. 验证标签存在
    // 2. 移除所有邮件与该标签的关联
    // 3. 删除标签记录
    // 4. 更新统计信息
    service.delete_label(id).await
}

///
/// 为邮件添加标签
///
/// 为指定邮件添加一个标签。
/// 如果邮件已经有该标签，此操作不会重复添加（幂等操作）。
///
/// 功能说明：
/// 1. 验证邮件和标签存在
/// 2. 检查邮件是否已经有该标签
/// 3. 如果没有，则添加标签关联
/// 4. 更新数据库
///
/// 参数：
/// - service: LabelService 实例
/// - email_id: 邮件 ID
/// - label_id: 标签 ID
///
/// 返回值：
/// - Ok(()): 添加成功
/// - Err(MailError):
///   - NotFound: 邮件或标签不存在
///   - DatabaseError: 数据库操作错误
///
/// 使用场景：
/// 1. 用户通过右键菜单为邮件添加标签时
/// 2. 用户拖放标签到邮件上时
/// 3. 批量操作为多封邮件添加标签时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// // 为邮件添加"工作"标签
/// await invoke('add_label_to_email', {
///   email_id: 123,
///   label_id: 1
/// });
/// console.log('标签已添加');
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn add_label_to_email(
    service: tauri::State<'_, LabelService>,
    email_id: i32,
    label_id: i32,
) -> Result<(), MailError> {
    // 记录调试日志
    tracing::debug!(email_id, label_id, "命令: 为邮件添加标签");

    // 调用服务层添加标签关联
    // 该方法会：
    // 1. 验证邮件和标签存在
    // 2. 检查是否已关联
    // 3. 添加关联（如果不存在）
    service.add_label_to_email(email_id, label_id).await
}

///
/// 从邮件移除标签
///
/// 从指定邮件中移除一个标签。
/// 如果邮件没有该标签，此操作不会报错（幂等操作）。
///
/// 功能说明：
/// 1. 验证邮件和标签存在
/// 2. 检查邮件是否有该标签
/// 3. 如果有，则移除标签关联
/// 4. 更新数据库
///
/// 参数：
/// - service: LabelService 实例
/// - email_id: 邮件 ID
/// - label_id: 标签 ID
///
/// 返回值：
/// - Ok(()): 移除成功
/// - Err(MailError):
///   - NotFound: 邮件或标签不存在
///   - DatabaseError: 数据库操作错误
///
/// 使用场景：
/// 1. 用户在邮件详情页点击标签的删除按钮时
/// 2. 用户取消邮件的某个标签时
/// 3. 批量操作移除多封邮件的标签时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// // 从邮件移除"工作"标签
/// await invoke('remove_label_from_email', {
///   email_id: 123,
///   label_id: 1
/// });
/// console.log('标签已移除');
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn remove_label_from_email(
    service: tauri::State<'_, LabelService>,
    email_id: i32,
    label_id: i32,
) -> Result<(), MailError> {
    // 记录调试日志
    tracing::debug!(email_id, label_id, "命令: 移除邮件标签");

    // 调用服务层移除标签关联
    // 该方法会：
    // 1. 验证邮件和标签存在
    // 2. 检查是否有关联
    // 3. 移除关联（如果存在）
    service.remove_label_from_email(email_id, label_id).await
}

///
/// 获取邮件的所有标签
///
/// 查询指定邮件的所有标签列表。
/// 此命令用于在邮件详情页或邮件列表中显示邮件的标签。
///
/// 功能说明：
/// 1. 验证邮件存在
/// 2. 查询邮件的所有标签关联
/// 3. 返回标签列表
///
/// 参数：
/// - service: LabelService 实例
/// - email_id: 邮件 ID
///
/// 返回值：
/// - Ok(Vec<LabelDto>): 标签列表，如果没有标签则返回空数组
/// - Err(MailError):
///   - NotFound: 邮件不存在
///   - DatabaseError: 数据库查询错误
///
/// 使用场景：
/// 1. 用户打开邮件详情页时
/// 2. 在邮件列表中显示邮件标签时
/// 3. 搜索带特定标签的邮件时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// const labels = await invoke('get_labels_for_email', { email_id: 123 });
/// if (labels.length === 0) {
///   console.log('此邮件没有标签');
/// } else {
///   labels.forEach(label => {
///     console.log(`标签: ${label.name}`);
///   });
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn get_labels_for_email(
    service: tauri::State<'_, LabelService>,
    email_id: i32,
) -> Result<Vec<LabelDto>, MailError> {
    // 记录调试日志
    tracing::debug!(email_id, "命令: 获取邮件标签列表");

    // 调用服务层获取邮件的标签
    // 该方法会：
    // 1. 验证邮件存在
    // 2. 查询标签关联
    // 3. 返回标签列表
    service.get_labels_for_email(email_id).await
}

///
/// 按标签列出邮件
///
/// 列出带有指定标签的所有邮件 ID。
/// 此命令用于快速查看某一类别的邮件。
///
/// 功能说明：
/// 1. 验证标签存在
/// 2. 查询所有关联该标签的邮件
/// 3. 返回邮件 ID 列表
///
/// 参数：
/// - service: LabelService 实例
/// - label_id: 标签 ID
///
/// 返回值：
/// - Ok(Vec<i32>): 邮件 ID 列表，按关联时间排序
/// - Err(MailError):
///   - NotFound: 标签不存在
///   - DatabaseError: 数据库查询错误
///
/// 扩展功能（可选）：
/// - 可以支持分页（在服务层实现）
/// - 可以支持排序（按日期、重要性等）
/// - 可以返回邮件的简要信息而不只是 ID
///
/// 使用场景：
/// 1. 用户点击标签查看相关邮件时
/// 2. 标签筛选功能
/// 3. 统计某一类邮件的数量
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// const emailIds = await invoke('list_emails_by_label', { label_id: 1 });
/// console.log(`找到 ${emailIds.length} 封带有该标签的邮件`);
///
/// // 可以结合其他命令获取邮件详情
/// for (const id of emailIds) {
///   const email = await invoke('get_email', { id });
///   console.log(email.subject);
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn list_emails_by_label(
    service: tauri::State<'_, LabelService>,
    label_id: i32,
) -> Result<Vec<i32>, MailError> {
    // 记录调试日志
    tracing::debug!(label_id, "命令: 按标签列出邮件");

    // 调用服务层获取带有该标签的邮件
    // 该方法会：
    // 1. 验证标签存在
    // 2. 查询邮件关联
    // 3. 返回邮件 ID 列表
    service.list_emails_by_label(label_id).await
}

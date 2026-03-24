//! IMAP 数据类型定义
//!
//! 定义了 IMAP 协议中使用的各种数据结构，包括：
//! - 邮件标志
//! - 邮件数据
//! - 文件夹信息
//! - IDLE 相关类型

use chrono::{DateTime, Utc};

// 注意：ImapAuth 已移至 auth.rs 模块

/// 邮件标志
///
/// 表示 IMAP 邮件系统标志（System Flags）和用户关键字。
///
/// # IMAP 标志说明
///
/// 根据 RFC 3501，系统标志包括：
/// - `\Seen`: 邮件已被阅读
/// - `\Answered`: 邮件已被回复
/// - `\Flagged`: 邮件已被标记为重要
/// - `\Deleted`: 邮件已标记为删除
/// - `\Draft`: 邮件是草稿
/// - `\Recent`: 邮件是最近的（此会话中新到达的）
///
/// # 字段说明
///
/// - `seen`: 对应 `\Seen` 标志，邮件是否已读
/// - `flagged`: 对应 `\Flagged` 标志，邮件是否被星标/标记
/// - `answered`: 对应 `\Answered` 标志，邮件是否已回复
/// - `deleted`: 对应 `\Deleted` 标志，邮件是否被标记删除
#[derive(Debug, Clone)]
pub struct EmailFlags {
    /// 邮件是否已读（\Seen）
    pub seen: bool,
    /// 邮件是否被标记/星标（\Flagged）
    pub flagged: bool,
    /// 邮件是否已回复（\Answered）
    pub answered: bool,
    /// 邮件是否被标记删除（\Deleted）
    pub deleted: bool,
}

/// 附件信息
///
/// 表示邮件附件的基本信息。
///
/// # 字段说明
///
/// - `filename`: 附件文件名（可能包含 MIME 编码）
/// - `content_type`: MIME 内容类型（如 "application/pdf"）
/// - `size`: 附件大小（字节）
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    /// 附件文件名
    pub filename: String,
    /// MIME 内容类型
    pub content_type: String,
    /// 附件大小（字节）
    pub size: u64,
}

/// 邮件数据
///
/// 表示一封完整的邮件，包含所有重要信息和内容。
///
/// # 字段说明
///
/// ## 基本信息
/// - `uid`: IMAP UID（在文件夹内唯一标识邮件）
/// - `subject`: 邮件主题
/// - `from`: 发件人地址
/// - `to`: 收件人地址列表
/// - `cc`: 抄送地址列表
/// - `date`: 邮件日期
///
/// ## 内容
/// - `body_text`: 纯文本正文
/// - `body_html`: HTML 正文
/// - `raw`: 原始邮件源码
///
/// ## 其他
/// - `flags`: 邮件标志
/// - `attachments`: 附件列表
///
/// # 性能注意
///
/// `raw` 字段包含完整的邮件源码，可能非常大。
/// 如果不需要原始数据，可以考虑使用 `EmailHeader` 代替。
#[derive(Debug, Clone)]
pub struct EmailData {
    /// IMAP UID
    pub uid: u32,
    /// 邮件主题
    pub subject: String,
    /// 发件人
    pub from: String,
    /// 收件人
    pub to: String,
    /// 抄送
    pub cc: String,
    /// 邮件日期
    pub date: DateTime<Utc>,
    /// 纯文本正文
    pub body_text: String,
    /// HTML 正文
    pub body_html: String,
    /// 原始邮件源码
    pub raw: String,
    /// 邮件标志
    pub flags: EmailFlags,
    /// 附件列表
    pub attachments: Vec<EmailAttachment>,
}

/// RFC 6154 Special-Use Mailboxes 属性
///
/// 定义了特殊用途文件夹的标准属性，用于自动识别常见的文件夹类型。
///
/// # 映射关系
///
/// | 属性 | IMAP 标志 | 用途 |
/// |------|-----------|------|
/// | `All` | \All | 所有邮件（虚拟文件夹） |
/// | `Archive` | \Archive | 归档邮件 |
/// | `Drafts` | \Drafts | 草稿 |
/// | `Flagged` | \Flagged | 已标记邮件 |
/// | `Junk` | \Junk | 垃圾邮件 |
/// | `Sent` | \Sent | 已发送 |
/// | `Trash` | \Trash | 已删除 |
///
/// # 示例
///
/// ```rust
/// // 检查文件夹是否为垃圾邮件文件夹
/// if folder_info.special_use == Some(SpecialUse::Junk) {
///     println!("这是垃圾邮件文件夹");
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialUse {
    /// 所有邮件 (\All)
    All,
    /// 归档 (\Archive)
    Archive,
    /// 草稿 (\Drafts)
    Drafts,
    /// 已标记/星标 (\Flagged)
    Flagged,
    /// 垃圾邮件 (\Junk)
    Junk,
    /// 已发送 (\Sent)
    Sent,
    /// 已删除 (\Trash)
    Trash,
}

/// 文件夹信息（包含 RFC 6154 special-use 属性）
///
/// 表示 IMAP 文件夹的完整信息。
///
/// # 字段说明
///
/// ## 标识符
/// - `name`: 文件夹的原始 IMAP 名称（可能包含编码）
/// - `standard_name`: 标准化的文件夹名称（基于 special-use 或常见模式）
///
/// ## 属性
/// - `special_use`: RFC 6154 特殊用途属性（如果有）
///
/// # 名称映射规则
///
/// `standard_name` 通过以下规则生成：
/// 1. 如果有 `special_use` 属性，使用对应的标准名称
/// 2. 否则，根据 `name` 的常见模式推断（如 "INBOX" → "INBOX"）
/// 3. 实在无法推断，使用原始名称
///
/// # 示例
///
/// ```rust
/// // Gmail 的 "[Gmail]/Sent Mail"
/// FolderInfo {
///     name: "[Gmail]/Sent Mail".to_string(),
///     special_use: Some(SpecialUse::Sent),
///     standard_name: "Sent".to_string(),
/// }
///
/// // Outlook 的 "Deleted Items"
/// FolderInfo {
///     name: "Deleted Items".to_string(),
///     special_use: Some(SpecialUse::Trash),
///     standard_name: "Trash".to_string(),
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FolderInfo {
    /// 文件夹名称（原始 IMAP 名称）
    pub name: String,
    /// RFC 6154 special-use 属性（如果有）
    pub special_use: Option<SpecialUse>,
    /// 标准名称（基于 special-use 或名称映射）
    pub standard_name: String,
}

/// 文件夹 IMAP 元数据
///
/// 包含文件夹的 IMAP 协议元数据，用于增量同步和状态管理。
///
/// # 字段说明
///
/// ## UID 相关
/// - `uidvalidity`: UIDVALIDITY 值，文件夹 UID 的有效性标识符
/// - `uidnext`: 预期的下一个 UID
///
/// ## 统计信息
/// - `exists`: 文件夹中存在的邮件数量
/// - `recent`: 最近邮件数量（\Recent 标志）
/// - `unseen`: 未读邮件数量（如果可用）
///
/// # UIDVALIDITY 说明
///
/// UIDVALIDITY 是一个递增的数字，每当文件夹的内容
/// 发生不可逆变化时会改变（如文件夹被删除后重新创建）。
/// 如果 UIDVALIDITY 改变，之前存储的 UID 不再有效。
#[derive(Debug, Clone)]
pub struct FolderMetadata {
    /// IMAP UIDVALIDITY 值 - 文件夹 UID 的有效性标识符
    pub uidvalidity: u64,
    /// 预期的下一个 UID
    pub uidnext: u64,
    /// 文件夹中存在的邮件数量
    pub exists: u32,
    /// 最近邮件数量
    pub recent: u32,
    /// 未读邮件数量（如果可用）
    pub unseen: Option<u32>,
}

/// 邮件头信息（仅包含邮件头，用于骨架同步）
///
/// 轻量级的邮件表示，仅包含邮件头和标志信息。
/// 适用于列表展示，不需要下载完整的邮件正文。
///
/// # 字段说明
///
/// 与 `EmailData` 相比，缺少以下字段：
/// - `body_text`: 纯文本正文
/// - `body_html`: HTML 正文
/// - `raw`: 原始邮件源码
/// - `attachments`: 附件列表
///
/// # 性能优势
///
/// - 减少数据传输量
/// - 加快列表加载速度
/// - 降低内存使用
///
/// # 使用场景
///
/// - 邮件列表展示
/// - 搜索结果预览
/// - 骨架同步（Skeleton Sync）
#[derive(Debug, Clone)]
pub struct EmailHeader {
    /// IMAP UID
    pub uid: u32,
    /// 邮件主题
    pub subject: String,
    /// 发件人
    pub from: String,
    /// 收件人
    pub to: String,
    /// 抄送
    pub cc: String,
    /// 邮件日期
    pub date: DateTime<Utc>,
    /// 邮件标志
    pub flags: EmailFlags,
}

// ========== IMAP IDLE 支持 (RFC 2177) ==========

/// IDLE 事件
///
/// 表示服务器通过 IDLE 推送的变更事件。
/// IDLE 是 IMAP 的扩展（RFC 2177），允许服务器主动推送邮件变更。
///
/// # 事件类型
///
/// ## 邮件相关事件
/// - `NewEmail`: 新邮件到达
/// - `FlagsChanged`: 邮件标志变更（已读、星标等）
/// - `EmailDeleted`: 邮件被删除
///
/// ## 连接相关事件
/// - `Disconnected`: 连接断开
/// - `Error`: 发生错误
///
/// # 使用场景
///
/// - 实时接收新邮件通知
/// - 多设备同步邮件状态
/// - 保持与服务器连接活跃
///
/// # 示例
///
/// ```rust
/// match event {
///     IdleEvent::NewEmail { folder, uid } => {
///         println!("新邮件到达 {}，UID: {}", folder, uid);
///     }
///     IdleEvent::FlagsChanged { folder, uid, flags } => {
///         println!("邮件标志变更: {} in {}", uid, folder);
///     }
///     _ => {}
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum IdleEvent {
    /// 新邮件到达
    NewEmail {
        /// 文件夹名称
        folder: String,
        /// 邮件 UID
        uid: u32,
    },
    /// 邮件标志变更
    FlagsChanged {
        /// 文件夹名称
        folder: String,
        /// 邮件 UID
        uid: u32,
        /// 变更后的标志列表
        flags: Vec<String>,
    },
    /// 邮件被删除
    EmailDeleted {
        /// 文件夹名称
        folder: String,
        /// 邮件 UID
        uid: u32,
    },
    /// 连接断开
    Disconnected,
    /// 发生错误
    Error(String),
}

/// IDLE 状态
///
/// 表示 IDLE 连接的当前状态。
///
/// # 状态转换图
///
/// ```text
/// Disconnected → Connected → IdleActive ↔ IdlePaused
///       ↑            ↓            ↓
///       └────────── Reconnect ───────┘
/// ```
///
/// # 状态说明
///
/// - `Disconnected`: 未连接，需要建立连接
/// - `Connected`: 已连接但未进入 IDLE 模式
/// - `IdleActive`: IDLE 模式激活，正在等待服务器推送
/// - `IdlePaused`: IDLE 暂停，正在处理服务器响应（如发送命令）
/// - `Reconnecting`: 正在尝试重新连接
#[derive(Clone, Debug, PartialEq)]
pub enum IdleState {
    /// 未连接
    Disconnected,
    /// 已连接，未进入 IDLE
    Connected,
    /// IDLE 模式激活中
    IdleActive,
    /// IDLE 暂停（处理服务器响应）
    IdlePaused,
    /// 正在重连
    Reconnecting,
}

/// 重连配置
///
/// 控制 IDLE 连接的自动重连行为。
///
/// # 字段说明
///
/// - `enabled`: 是否启用自动重连
/// - `max_attempts`: 最大重试次数
/// - `initial_delay_secs`: 初始重连延迟（秒）
/// - `max_delay_secs`: 最大重连延迟（秒）
/// - `backoff_multiplier`: 退避指数
///
/// # 重连策略
///
/// 使用指数退避算法，每次重连后延迟时间乘以 `backoff_multiplier`：
///
/// ```
/// delay(n) = min(initial_delay * backoff_multiplier^n, max_delay)
/// ```
///
/// # 默认配置
///
/// - 启用自动重连
/// - 最多重试 10 次
/// - 初始延迟 5 秒
/// - 最大延迟 5 分钟（300 秒）
/// - 退避指数 2.0
///
/// # 示例
///
/// ```rust
/// let config = ReconnectConfig {
///     enabled: true,
///     max_attempts: 5,
///     initial_delay_secs: 10,
///     max_delay_secs: 60,
///     backoff_multiplier: 1.5,
/// };
/// // 延迟序列: 10s, 15s, 22.5s, 33.75s, 50.6s, 60s (上限)
/// ```
#[derive(Clone, Debug)]
pub struct ReconnectConfig {
    /// 是否启用自动重连
    pub enabled: bool,
    /// 最大重试次数
    pub max_attempts: u32,
    /// 初始重连延迟（秒）
    pub initial_delay_secs: u64,
    /// 最大重连延迟（秒）
    pub max_delay_secs: u64,
    /// 退避指数
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 10,
            initial_delay_secs: 5,
            max_delay_secs: 300,  // 5分钟
            backoff_multiplier: 2.0,
        }
    }
}

/// IDLE 句柄
///
/// 用于控制 IDLE 连接的生命周期。
///
/// # 功能
///
/// - 启动和停止 IDLE 连接
/// - 检查连接是否活跃
/// - 线程安全的状态管理
///
/// # 字段说明
///
/// - `folder`: 监听的文件夹名称
/// - `active`: 原子布尔值，表示是否应该继续运行
/// - `account_id`: 关联的账号 ID
///
/// # 使用示例
///
/// ```rust
/// // 创建句柄
/// let handle = IdleHandle::new("INBOX".to_string(), 1);
///
/// // 在后台线程中使用
/// thread::spawn(move || {
///     while handle.is_active() {
///         // 处理 IDLE 事件
///     }
/// });
///
/// // 停止 IDLE
/// handle.stop();
/// ```
#[derive(Clone)]
pub struct IdleHandle {
    /// 文件夹名称
    pub folder: String,
    /// 是否处于活跃状态
    pub active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 账号ID
    pub account_id: i32,
}

impl IdleHandle {
    /// 创建新的 IDLE 句柄
    ///
    /// # 参数
    ///
    /// - `folder`: 要监听的文件夹名称
    /// - `account_id`: 关联的账号 ID
    ///
    /// # 返回
    ///
    /// 返回一个新的 `IdleHandle`，初始状态为活跃。
    pub fn new(folder: String, account_id: i32) -> Self {
        Self {
            folder,
            active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            account_id,
        }
    }

    /// 停止 IDLE
    ///
    /// 将活跃状态设置为 false，信号通知 IDLE 循环退出。
    /// 此方法是线程安全的，可以从任何线程调用。
    pub fn stop(&self) {
        self.active.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// 检查是否活跃
    ///
    /// 返回当前的活跃状态。
    /// 此方法是线程安全的，可以从任何线程调用。
    ///
    /// # 返回
    ///
    /// - `true`: IDLE 应该继续运行
    /// - `false`: IDLE 应该停止
    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }
}

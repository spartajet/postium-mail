//! 邮件列表状态（移植自 src/lib/stores/email.svelte.ts 的列表装载部分）。
//!
//! 双行模型（Task 8）：`view_mode` 决定列表行数据来自 `conversations`
//! （会话档，线程聚合行）还是 `emails`（平铺档，单封邮件行）。会话档
//! 下搜索降级为平铺结果执行（FTS 会话聚合非本期）；草稿/垃圾/回收站
//! 与标签视图恒走平铺装载（见 [`EmailsStore::in_conversation_view`]）。
//!
//! 竞态防护：所有异步装载携带代数（`load_gen`），完成回调先比对代数，
//! 过期结果（导航/搜索/切档已切换）直接丢弃——避免旧数据落入新视图。
//! `loading` 在任何分支（含 Err/通道关闭）都会复位，不会卡死。

use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::service::email_service::{
    ConversationListResponse, ConversationSummary, EmailCategory, EmailDto,
};

use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;

/// 邮件列表视图档位。
///
/// - [`EmailViewMode::Threaded`]：会话（线程）聚合行——行数据在
///   [`EmailsStore::conversations`]，选中态走 `selected_thread`；
/// - [`EmailViewMode::Flat`]：单封邮件平铺行（既有默认）——行数据在
///   [`EmailsStore::emails`]，选中态走 `selected`。
#[derive(Clone, Copy, PartialEq)]
pub enum EmailViewMode {
    Threaded,
    Flat,
}

pub struct EmailsStore {
    /// 当前导航目标（装载依据）
    pub nav: NavTarget,
    /// 当前页邮件（平铺档行数据，分页追加；搜索结果亦落此处）
    pub emails: Vec<EmailDto>,
    pub total: u64,
    /// 当前页会话（会话档行数据，分页追加）
    pub conversations: Vec<ConversationSummary>,
    /// 会话总数（会话档 load_more 分页闸门）
    pub conversation_total: u64,
    pub page: usize,
    pub loading: bool,
    /// 选中的邮件 id（详情视图用；与 selected_thread 互斥）
    pub selected: Option<i32>,
    /// 选中变化计数（宿主据此触发详情加载）
    pub selected_version: u32,
    /// 选中的线程 id（会话时间线用；与 selected 互斥）
    pub selected_thread: Option<i64>,
    /// 会话选中变化计数（宿主据此装载会话时间线）
    #[allow(dead_code)] // Task 10 起消费（宿主接线会话时间线）
    pub selected_thread_version: u32,
    /// 列表数据版本（每次数据变化 +1；宿主据此原位更新行数据）
    pub list_version: u64,
    /// 视图上下文版本（仅在导航/搜索切换时 +1；宿主据此重建整个列表，
    /// 与 list_version 分离使翻页/刷新不重置滚动与选中）
    pub nav_version: u64,
    /// 未读过滤
    pub unread_only: bool,
    /// 当前 FTS 搜索词（None = 普通导航视图）；refresh_now 保持在搜索上下文内
    pub search_query: Option<String>,
    /// 当前视图档位（默认 Flat，计划 T12 裁定；new 从 ui-state 恢复记忆值）
    pub view_mode: EmailViewMode,
    /// 装载代数：每次 navigate/search/refresh 递增，完成回调据此丢弃过期结果
    load_gen: u64,
}

#[derive(Clone, Copy)]
pub enum EmailsEvent {
    Reloaded,
    /// 选中邮件变化（详情据此装载）
    SelectionChanged(i32),
    /// 选中会话变化（时间线据此装载；与 SelectionChanged 互斥）。
    /// 载荷 thread_id 由 T10 宿主时间线装载消费。
    #[allow(dead_code)] // Task 10 起消费（宿主装载会话时间线读载荷）
    ThreadSelectionChanged(i64),
    /// 未读状态变化（标已读/未读后，侧栏徽章据此刷新文件夹统计）
    UnreadChanged,
}

impl EventEmitter<EmailsEvent> for EmailsStore {}

pub const PAGE_LIMIT: usize = 50;

impl EmailsStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        // 默认 Flat（计划 T12 裁定）；用户切过会话档则从 ui-state 恢复
        // （缺省/未知值宽容降级 Flat，与 ui_state::load 的降级口径一致）
        let view_mode = match crate::ui_state::load().email_view_mode.as_deref() {
            Some("threaded") => EmailViewMode::Threaded,
            _ => EmailViewMode::Flat,
        };
        Self {
            nav: NavTarget::AllInbox,
            emails: Vec::new(),
            total: 0,
            conversations: Vec::new(),
            conversation_total: 0,
            page: 0,
            loading: false,
            selected: None,
            selected_version: 0,
            selected_thread: None,
            selected_thread_version: 0,
            list_version: 0,
            nav_version: 0,
            unread_only: false,
            search_query: None,
            view_mode,
            load_gen: 0,
        }
    }

    /// 导航变化时重置并装载第一页（退出搜索上下文）。
    pub fn navigate(&mut self, nav: NavTarget, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.nav = nav;
        self.search_query = None;
        self.reset_and_load(api, cx);
    }

    /// 切换视图档位：持久化到 ui-state + 重置装载当前导航。
    ///
    /// 与 navigate 同语义退出搜索上下文（搜索结果本就与档位无关——
    /// 会话档搜索降级为平铺结果，见 [`Self::search`]），切档即回到
    /// 导航视图重装。同档位重复调用为幂等空操作（不触发重载）。
    pub fn set_view_mode(
        &mut self,
        mode: EmailViewMode,
        api: &Arc<CoreApi>,
        cx: &mut Context<Self>,
    ) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        let mut ui = crate::ui_state::load();
        ui.email_view_mode = Some(
            match mode {
                EmailViewMode::Threaded => "threaded",
                EmailViewMode::Flat => "flat",
            }
            .to_string(),
        );
        crate::ui_state::save(&ui);
        self.search_query = None;
        self.reset_and_load(api, cx);
    }

    /// 选中会话（会话档列表行点击）。与单封邮件选中互斥：清空 `selected`，
    /// 详情区域由宿主按 `selected_thread_version` 切换为会话时间线。
    /// 反向互斥（平铺选中清会话选中）由列表交互层接线（行集合按档位
    /// 整体替换，`set_rows`/`set_conversations` 各自清对侧选中）。
    pub fn select_thread(&mut self, thread_id: i64, cx: &mut Context<Self>) {
        let changed = self.selected_thread != Some(thread_id);
        self.selected_thread = Some(thread_id);
        self.selected_thread_version += 1;
        // 互斥：清邮件选中，宿主邮件详情让位于会话时间线
        self.selected = None;
        self.selected_version += 1;
        cx.notify();
        if changed {
            cx.emit(EmailsEvent::ThreadSelectionChanged(thread_id));
        }
    }

    /// 当前列表上下文是否装载会话行（列表渲染层与宿主分派行数据源的
    /// 同一依据）：会话档 + 非搜索态 + 导航目标有对应会话数据。
    ///
    /// 回退平铺的情形（T8 承接裁定，Task 9）：
    /// - 标签视图：无会话聚合 API，v1 回退平铺装载；
    /// - 草稿/垃圾/回收站（分类或具体文件夹）：上游会话查询恒过滤
    ///   `is_draft = 0 AND is_deleted = 0`，这些视图的会话档结果恒为
    ///   空——store 层直接保持平铺装载，列表才能看到数据。
    pub fn in_conversation_view(&self) -> bool {
        self.view_mode == EmailViewMode::Threaded
            && self.search_query.is_none()
            && match &self.nav {
                NavTarget::AllInbox => true,
                NavTarget::Category(category) => !matches!(
                    category,
                    EmailCategory::Drafts | EmailCategory::Spam | EmailCategory::Trash
                ),
                NavTarget::AccountFolder { folder, .. } => !matches!(
                    crate::nav::folder_label_key(folder),
                    Some("sidebar.drafts") | Some("sidebar.spam") | Some("sidebar.trash")
                ),
                _ => false,
            }
    }

    /// 追加下一页。
    pub fn load_more(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        // 分页闸门按当前上下文的行集合（会话 or 邮件）判定
        let reached_end = if self.in_conversation_view() {
            (self.conversations.len() as u64) >= self.conversation_total
        } else {
            (self.emails.len() as u64) >= self.total
        };
        if reached_end {
            return;
        }
        let page = self.page + 1;
        if self.in_conversation_view() {
            self.load_conversations(page, api, cx);
        } else {
            self.load(page, api, cx);
        }
    }

    /// 按当前上下文（导航或搜索）重载第一页（操作后刷新）。
    pub fn refresh_now(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if let Some(q) = self.search_query.clone() {
            self.search(&q, api, cx);
        } else {
            self.reset_and_load(api, cx);
        }
    }

    fn reset_and_load(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.nav_version += 1; // 上下文切换：宿主需重建列表
        self.emails.clear();
        self.total = 0;
        self.conversations.clear();
        self.conversation_total = 0;
        self.page = 0;
        self.selected = None;
        self.selected_version += 1;
        self.selected_thread = None;
        self.selected_thread_version += 1;
        if self.in_conversation_view() {
            self.load_conversations(0, api, cx);
        } else {
            self.load(0, api, cx);
        }
    }

    /// FTS 搜索（跨当前范围）。
    ///
    /// 会话档降级裁定（计划 T8）：FTS 会话聚合非本期，搜索一律装载
    /// 平铺结果（落 `emails`），`view_mode` 保持不变——列表渲染层
    /// （T9）按「搜索态强制平铺行」处理（搜索态可由
    /// `search_query.is_some()` 推导，不另设状态位）。
    pub fn search(&mut self, query: &str, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        let q = query.trim().to_string();
        if q.is_empty() {
            self.navigate(self.nav.clone(), api, cx);
            return;
        }
        self.load_gen += 1;
        let load_g = self.load_gen;
        self.search_query = Some(q.clone());
        self.nav_version += 1; // 搜索 = 新上下文：重建列表
        self.loading = true;
        // 搜索切换上下文：清选中（旧选中在结果中不可见）
        self.selected = None;
        self.selected_version += 1;
        // 会话档搜索降级为平铺行：清会话上下文，避免旧会话行与
        // 搜索结果并存（与 selected 清空同口径）
        self.conversations.clear();
        self.conversation_total = 0;
        self.selected_thread = None;
        self.selected_thread_version += 1;
        self.page = 0;
        cx.notify();
        let api = api.clone();
        let q_for_log = q.clone();
        let rx = spawn_tokio(async move { api.search_emails(&q, None, Some(100)).await });
        cx.spawn(async move |this, cx| {
            // recv Err（任务 panic/通道关闭）也走到统一收尾，loading 必复位
            let result = rx.recv().await.ok().and_then(|r| r.ok());
            if result.is_none() {
                tracing::warn!(target: "postium::ui", query = %q_for_log, "邮件搜索失败");
            }
            this.update(cx, |store, cx| {
                if store.load_gen != load_g {
                    return; // 过期结果：用户已导航/再次搜索
                }
                match result {
                    Some(rows) => {
                        store.emails = rows
                            .into_iter()
                            .map(|r| EmailDto {
                                id: r.id,
                                account_id: r.account_id,
                                folder: r.folder.clone(),
                                uid: 0,
                                subject: r.subject.clone(),
                                sender_name: None,
                                sender_email: r.sender_email.clone(),
                                account_email: r.account_email.clone(),
                                account_display_name: r.account_display_name.clone(),
                                preview: r.preview.clone(),
                                is_read: r.is_read,
                                is_starred: r.is_starred,
                                sent_at: r.sent_at,
                                has_attachments: false,
                                // 搜索结果不携带头信息，两个头字段占位 None
                                message_id: None,
                                in_reply_to: None,
                            })
                            .collect();
                        store.total = store.emails.len() as u64;
                        store.page = 0;
                        store.list_version += 1;
                    }
                    None => {
                        // 查询失败：回到空列表并退出搜索态，避免工具条操作不可见选中
                        store.emails.clear();
                        store.total = 0;
                        store.search_query = None;
                        store.list_version += 1;
                    }
                }
                store.loading = false;
                cx.emit(EmailsEvent::Reloaded);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load(&mut self, page: usize, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.load_gen += 1;
        let load_g = self.load_gen;
        self.loading = true;
        cx.notify();

        let nav = self.nav.clone();
        let unread_only = self.unread_only;
        let api = api.clone();
        let rx = spawn_tokio(async move {
            match &nav {
                NavTarget::AllInbox => {
                    api.list_emails_by_category_for_all_accounts(
                        EmailCategory::Inbox,
                        page,
                        PAGE_LIMIT,
                        unread_only,
                    )
                    .await
                }
                NavTarget::Category(category) => {
                    // 聚合分类视图：用跨账号接口（单账号语义由 AccountFolder 承担）
                    api.list_emails_by_category_for_all_accounts(
                        category.clone(),
                        page,
                        PAGE_LIMIT,
                        unread_only,
                    )
                    .await
                }
                NavTarget::AccountFolder { account_id, folder } => {
                    api.list_emails(*account_id, folder, page, PAGE_LIMIT).await
                }
                NavTarget::Label(label_id) => {
                    // 标签视图：取标签下全部 id，按页切片取详情摘要
                    let ids = api.list_emails_by_label(*label_id).await.unwrap_or_else(|e| {
                        tracing::warn!(target: "postium::ui", "加载标签邮件失败: {e}");
                        Vec::new()
                    });
                    let start = page * PAGE_LIMIT;
                    let mut out = Vec::new();
                    for id in ids.iter().skip(start).take(PAGE_LIMIT) {
                        if let Ok(detail) = api.get_email(*id).await {
                            out.push(detail_to_dto(&detail));
                        }
                    }
                    Ok(postium_core::service::email_service::EmailListResponse {
                        emails: out,
                        total: ids.len() as u64,
                        page,
                        limit: PAGE_LIMIT,
                    })
                }
                _ => Ok(postium_core::service::email_service::EmailListResponse {
                    emails: Vec::new(),
                    total: 0,
                    page,
                    limit: PAGE_LIMIT,
                }),
            }
        });

        cx.spawn(async move |this, cx| {
            let result = rx.recv().await.ok().and_then(|r| r.ok());
            if result.is_none() {
                tracing::warn!(target: "postium::ui", page, "邮件列表加载失败（保持现有数据）");
            }
            this.update(cx, |store, cx| {
                if store.load_gen != load_g {
                    return; // 过期结果：导航/搜索已切换
                }
                match result {
                    Some(resp) => {
                        if page == 0 {
                            store.emails = resp.emails;
                        } else {
                            store.emails.extend(resp.emails);
                        }
                        store.total = resp.total;
                        store.page = page;
                        store.list_version += 1;
                    }
                    None => {
                        // 失败：保持现有数据，仅复位 loading（下次可重试）
                        if page == 0 && store.emails.is_empty() {
                            store.list_version += 1;
                        }
                    }
                }
                store.loading = false;
                cx.emit(EmailsEvent::Reloaded);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 会话档装载（[`Self::load`] 的会话聚合对应物，同形态分派）。
    ///
    /// 上游会话 API 页码从 1 开始（store 内部统一 0 基），调用侧 +1 对齐。
    /// 未读过滤三变体统一下推（会话 API 均带 unread_only 参数）。
    fn load_conversations(&mut self, page: usize, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.load_gen += 1;
        let load_g = self.load_gen;
        self.loading = true;
        cx.notify();

        let nav = self.nav.clone();
        let unread_only = self.unread_only;
        let api = api.clone();
        let page_1based = page + 1;
        let rx = spawn_tokio(async move {
            match &nav {
                NavTarget::AllInbox => {
                    api.list_conversations_by_category_for_all_accounts(
                        EmailCategory::Inbox,
                        page_1based,
                        PAGE_LIMIT,
                        unread_only,
                    )
                    .await
                }
                NavTarget::Category(category) => {
                    // 聚合分类视图：与平铺路径同口径用跨账号接口
                    // （单账号语义由 AccountFolder 承担）
                    api.list_conversations_by_category_for_all_accounts(
                        category.clone(),
                        page_1based,
                        PAGE_LIMIT,
                        unread_only,
                    )
                    .await
                }
                NavTarget::AccountFolder { account_id, folder } => {
                    api.list_conversations(*account_id, folder, page_1based, PAGE_LIMIT, unread_only)
                        .await
                }
                // in_conversation_view 已过滤：其余导航目标不会进入会话
                // 装载；防御性返回空页（与 load 的兜底分支同形态）
                _ => Ok(ConversationListResponse {
                    conversations: Vec::new(),
                    total: 0,
                    page: page_1based,
                    limit: PAGE_LIMIT,
                }),
            }
        });

        cx.spawn(async move |this, cx| {
            let result = rx.recv().await.ok().and_then(|r| r.ok());
            if result.is_none() {
                tracing::warn!(target: "postium::ui", page, "会话列表加载失败（保持现有数据）");
            }
            this.update(cx, |store, cx| {
                if store.load_gen != load_g {
                    return; // 过期结果：导航/搜索/切档已切换
                }
                match result {
                    Some(resp) => {
                        if page == 0 {
                            store.conversations = resp.conversations;
                        } else {
                            store.conversations.extend(resp.conversations);
                        }
                        store.conversation_total = resp.total;
                        store.page = page;
                        store.list_version += 1;
                    }
                    None => {
                        // 失败：保持现有数据，仅复位 loading（下次可重试）
                        if page == 0 && store.conversations.is_empty() {
                            store.list_version += 1;
                        }
                    }
                }
                store.loading = false;
                cx.emit(EmailsEvent::Reloaded);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// 详情 → 列表行 DTO（标签视图用）。
fn detail_to_dto(detail: &postium_core::service::email_service::EmailDetail) -> EmailDto {
    let mut dto = detail.email.clone();
    dto.has_attachments = !detail.attachments.is_empty();
    dto
}

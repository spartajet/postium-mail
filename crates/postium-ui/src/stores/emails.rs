//! 邮件列表状态（移植自 src/lib/stores/email.svelte.ts 的列表装载部分）。
//!
//! 竞态防护：所有异步装载携带代数（`load_gen`），完成回调先比对代数，
//! 过期结果（导航/搜索已切换）直接丢弃——避免旧数据落入新视图。
//! `loading` 在任何分支（含 Err/通道关闭）都会复位，不会卡死。

use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::service::email_service::{EmailCategory, EmailDto};

use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;

pub struct EmailsStore {
    /// 当前导航目标（装载依据）
    pub nav: NavTarget,
    /// 当前页邮件（分页追加）
    pub emails: Vec<EmailDto>,
    pub total: u64,
    pub page: usize,
    pub loading: bool,
    /// 选中的邮件 id（详情视图用）
    pub selected: Option<i32>,
    /// 选中变化计数（宿主据此触发详情加载）
    pub selected_version: u32,
    /// 列表数据版本（每次数据变化 +1；宿主据此原位更新行数据）
    pub list_version: u64,
    /// 视图上下文版本（仅在导航/搜索切换时 +1；宿主据此重建整个列表，
    /// 与 list_version 分离使翻页/刷新不重置滚动与选中）
    pub nav_version: u64,
    /// 未读过滤
    pub unread_only: bool,
    /// 当前 FTS 搜索词（None = 普通导航视图）；refresh_now 保持在搜索上下文内
    pub search_query: Option<String>,
    /// 装载代数：每次 navigate/search/refresh 递增，完成回调据此丢弃过期结果
    load_gen: u64,
}

#[derive(Clone, Copy)]
pub enum EmailsEvent {
    Reloaded,
    /// 选中邮件变化（详情据此装载）
    SelectionChanged(i32),
    /// 未读状态变化（标已读/未读后，侧栏徽章据此刷新文件夹统计）
    UnreadChanged,
}

impl EventEmitter<EmailsEvent> for EmailsStore {}

pub const PAGE_LIMIT: usize = 50;

impl EmailsStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            nav: NavTarget::AllInbox,
            emails: Vec::new(),
            total: 0,
            page: 0,
            loading: false,
            selected: None,
            selected_version: 0,
            list_version: 0,
            nav_version: 0,
            unread_only: false,
            search_query: None,
            load_gen: 0,
        }
    }

    /// 导航变化时重置并装载第一页（退出搜索上下文）。
    pub fn navigate(&mut self, nav: NavTarget, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.nav = nav;
        self.search_query = None;
        self.reset_and_load(api, cx);
    }

    /// 追加下一页。
    pub fn load_more(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading || (self.emails.len() as u64) >= self.total {
            return;
        }
        let page = self.page + 1;
        self.load(page, api, cx);
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
        self.page = 0;
        self.selected = None;
        self.selected_version += 1;
        self.load(0, api, cx);
    }

    /// FTS 搜索（跨当前范围）。
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
}

/// 详情 → 列表行 DTO（标签视图用）。
fn detail_to_dto(detail: &postium_core::service::email_service::EmailDetail) -> EmailDto {
    let mut dto = detail.email.clone();
    dto.has_attachments = !detail.attachments.is_empty();
    dto
}

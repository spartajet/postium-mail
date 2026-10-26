//! 邮件列表状态（移植自 src/lib/stores/email.svelte.ts 的列表装载部分）。

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, EventEmitter};
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
    /// 列表数据版本（每次重载 +1，宿主 render 据此重建列表）
    pub list_version: u64,
}

#[derive(Clone, Copy)]
pub enum EmailsEvent {
    Reloaded,
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
        }
    }

    /// 导航变化时重置并装载第一页。
    pub fn navigate(&mut self, nav: NavTarget, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.nav = nav.clone();
        self.emails.clear();
        self.total = 0;
        self.page = 0;
        self.selected = None;
        self.load_page(0, api, cx);
    }

    /// 追加下一页。
    pub fn load_more(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading || (self.emails.len() as u64) >= self.total {
            return;
        }
        self.load_page(self.page + 1, api, cx);
    }

    /// FTS 搜索（跨当前范围）。
    pub fn search(&mut self, query: &str, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        let q = query.trim().to_string();
        if q.is_empty() {
            self.navigate(self.nav.clone(), api, cx);
            return;
        }
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();
        let api = api.clone();
        let rx = spawn_tokio(async move { api.search_emails(&q, None, Some(100)).await });
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                this.update(cx, |store, cx| {
                    if let Ok(rows) = result {
                        // SearchResult 与 EmailDto 字段相近，转换为展示模型
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
                                is_read: true,      // FTS 结果无已读字段，按已读显示
                                is_starred: false,  // 同上
                                sent_at: r.sent_at,
                                has_attachments: false,
                            })
                            .collect();
                        store.total = store.emails.len() as u64;
                        store.page = 0;
                        store.loading = false;
                        store.list_version += 1;
                        cx.emit(EmailsEvent::Reloaded);
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn load_page(&mut self, page: usize, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();

        let nav = self.nav.clone();
        let api = api.clone();
        let rx = spawn_tokio(async move {
            match &nav {
                NavTarget::AllInbox => {
                    api.list_emails_by_category_for_all_accounts(
                        EmailCategory::Inbox,
                        page,
                        PAGE_LIMIT,
                        false,
                    )
                    .await
                }
                NavTarget::Category(category) => {
                    // 聚合分类视图：用跨账号接口（单账号语义由 AccountFolder 承担）
                    api.list_emails_by_category_for_all_accounts(
                        category.clone(),
                        page,
                        PAGE_LIMIT,
                        false,
                    )
                    .await
                }
                NavTarget::AccountFolder { account_id, folder } => {
                    api.list_emails(*account_id, folder, page, PAGE_LIMIT).await
                }
                NavTarget::Label(label_id) => {
                    // 标签视图：取标签下的邮件 id，再取详情摘要（简化：直接按 id 拉取前 100 封）
                    let ids = api.list_emails_by_label(*label_id).await.unwrap_or_default();
                    let mut out = Vec::with_capacity(ids.len());
                    for id in ids.iter().take(100) {
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
            if let Ok(result) = rx.recv().await {
                this.update(cx, |store, cx| {
                    if let Ok(resp) = result {
                        if page == 0 {
                            store.emails = resp.emails;
                        } else {
                            store.emails.extend(resp.emails);
                        }
                        store.total = resp.total;
                        store.page = page;
                        store.list_version += 1;
                    }
                    store.loading = false;
                    cx.emit(EmailsEvent::Reloaded);
                    cx.notify();
                })
                .ok();
            }
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

//! 邮件详情视图（移植自 src/lib/components/email/EmailDetail.svelte）。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::ContactWrite;

use crate::i18n::{t, t_format};
use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;
use crate::stores::detail::DetailStore;
use crate::views::email_html::{ImageCtx, collect_remote_image_urls, render_html_document};

pub struct EmailDetailView {
    pub store: gpui_kit::Entity<DetailStore>,
    pub api: Arc<CoreApi>,
    /// 邮件列表弱引用：归档/删除/标未读后刷新列表（已删邮件不再留在列表）
    pub emails: gpui_kit::WeakEntity<crate::stores::emails::EmailsStore>,
    pub recipients_expanded: bool,
}

impl EmailDetailView {
    fn toggle_star(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let id = detail.email.id;
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.toggle_star(id).await });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(starred)) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.apply_star(starred);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn archive(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.archive_email(detail.email.id).await });
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(())) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.detail = None;
                    s.html = None;
                    cx.notify();
                });
                if let Some(list) = emails.upgrade() {
                    list.update(cx, |s, cx| {
                        s.refresh_now(&api_keep, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn delete(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokok_delete(api, detail.email.id);
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(_)) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.detail = None;
                    s.html = None;
                    cx.notify();
                });
                if let Some(list) = emails.upgrade() {
                    list.update(cx, |s, cx| {
                        s.refresh_now(&api_keep, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn reply_all(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        // 收件人 = 发件人 + 原收件人（去重）
        let mut to = vec![detail.email.sender_email.clone()];
        for r in detail.recipient_emails.split([',', ';']) {
            let r = r.trim();
            if !r.is_empty() && !to.iter().any(|x| x.eq_ignore_ascii_case(r)) {
                to.push(r.to_string());
            }
        }
        let to = to.join(", ");
        let subject = detail.email.subject.clone().unwrap_or_default();
        let original_html = detail.body_html.clone().unwrap_or_default();
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::ReplyAll {
                    email_id: detail.email.id,
                    account_id: detail.email.account_id,
                    to,
                    subject,
                    original_html,
                },
                &api,
                cx,
            );
        });
    }

    fn mark_unread(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let id = detail.email.id;
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.mark_as_read(id, false).await });
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(())) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.detail = None;
                    s.html = None;
                    cx.notify();
                });
                if let Some(list) = emails.upgrade() {
                    list.update(cx, |s, cx| {
                        s.refresh_now(&api_keep, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn reload(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let id = detail.email.id;
        let api = self.api.clone();
        let api_keep = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.reload_email(id).await });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(_)) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.force_load(id, &api_keep, cx);
                });
            }
        })
        .detach();
    }

    fn reply(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let to = detail.email.sender_email.clone();
        let subject = detail.email.subject.clone().unwrap_or_default();
        let original_html = detail.body_html.clone().unwrap_or_default();
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::Reply {
                    email_id: detail.email.id,
                    account_id: detail.email.account_id,
                    to,
                    subject,
                    original_html,
                },
                &api,
                cx,
            );
        });
    }

    fn forward(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let subject = detail.email.subject.clone().unwrap_or_default();
        let original_html = detail.body_html.clone().unwrap_or_default();
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::Forward {
                    email_id: detail.email.id,
                    account_id: detail.email.account_id,
                    subject,
                    original_html,
                },
                &api,
                cx,
            );
        });
    }

    fn open_attachment(
        &mut self,
        att_id: i32,
        _: &ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let api = self.api.clone();
        let _ = spawn_tokio(async move { api.open_attachment(att_id).await });
        let _ = cx;
    }

    fn save_attachment_as(
        &mut self,
        att_id: i32,
        filename: String,
        _: &ClickEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        // 文件对话框在后台线程（rfd async）
        let api = self.api.clone();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new().set_file_name(&filename).save_file() {
                let target = path.to_string_lossy().to_string();
                let _ = crate::runtime::runtime()
                    .spawn(async move { api.save_attachment_as(att_id, target).await });
            }
        });
    }

    /// 「加载图片」条点击（Task 13）：翻转放行态 + 逐图后台拉取。
    ///
    /// URL 清单取自缓存文档（渲染期已解析，此处缓存命中零重解析）；
    /// 状态与拉取编排都在 DetailStore（[`DetailStore::allow_remote_images`]）。
    /// 文档按浅色通道取（Remote 图 URL 不受深色适配影响，`ImageRef` 无颜色）。
    fn enable_remote_images(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some((email_id, html)) = ({
            let store = self.store.read(cx);
            store
                .detail
                .as_ref()
                .map(|d| (d.email.id, store.html.clone().unwrap_or_default()))
        }) else {
            return;
        };
        let Some(doc) = self
            .store
            .update(cx, |store, _| store.cached_doc(email_id, &html, false))
        else {
            return;
        };
        let urls = collect_remote_image_urls(&doc);
        let api = self.api.clone();
        self.store
            .update(cx, |store, cx| store.allow_remote_images(urls, &api, cx));
    }

    /// 浏览器精确查看（table 布局保真兜底）。
    fn open_in_browser(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        if let Some(body) = detail.body_html.clone() {
            // 写临时 HTML 并用系统默认浏览器打开
            let dir = std::env::temp_dir().join("postium-view");
            let _ = std::fs::create_dir_all(&dir);
            let file = dir.join(format!("email-{}.html", detail.email.id));
            let page = format!(
                "<!doctype html><meta charset=\"utf-8\"><body style=\"font-family:sans-serif\">{body}</body>"
            );
            if std::fs::write(&file, page).is_ok() {
                let _ = open::that(&file);
            }
        }
    }

    /// 操作行「联系人」联动：按发件人邮箱查联系人——已存在则经 MainShell
    /// 全局注册表切主窗口到联系人页并按邮箱选中（视图未建时由壳渲染期
    /// 消费待选邮箱）；不存在则一步创建 + toast「已添加」。
    pub(crate) fn add_contact(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else {
            return;
        };
        let sender = detail.email.sender_email.trim().to_string();
        if sender.is_empty() {
            return;
        }
        let sender_name = detail
            .email
            .sender_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string);
        let api = self.api.clone();
        let sender_link = sender.clone();
        let rx = spawn_tokio(async move {
            // 查 → 无则建：一条异步链完成判定，UI 侧只消费结果
            match api.get_contact_by_email(sender.clone()).await {
                Ok(Some(_)) => Ok(false),
                Ok(None) => api
                    .create_contact(ContactWrite {
                        email: sender,
                        extra_emails: None,
                        name: sender_name,
                        phones: None,
                        notes: None,
                        org: None,
                    })
                    .await
                    .map(|_| true),
                Err(e) => Err(e),
            }
        });
        cx.spawn(async move |_, cx| {
            let Ok(Ok(created)) = rx.recv().await else {
                return;
            };
            if created {
                crate::notify::toast(t("contacts.added"));
            } else {
                // 已存在：经全局弱注册表切联系人页，待选邮箱由壳渲染期消费
                cx.update(|cx| {
                    let Some(shell) = cx
                        .try_global::<crate::app::MainShell>()
                        .and_then(|g| g.0.upgrade())
                    else {
                        return;
                    };
                    shell.update(cx, |shell, cx| {
                        shell.nav = NavTarget::Contacts;
                        shell.contacts_pending_email = Some(sender_link.clone());
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }
}

fn spawn_tokok_delete(
    api: Arc<CoreApi>,
    id: i32,
) -> smol::channel::Receiver<Result<usize, postium_core::error::MailError>> {
    spawn_tokio(async move { api.delete_emails(vec![id]).await })
}

impl EmailDetailView {
    /// 详情中的纯文本正文（短借 store 取完即还——正文渲染需 `&mut App`，
    /// detail 共享借用不得跨越该调用）。
    fn detail_body_text(&self, cx: &App) -> String {
        self.store
            .read(cx)
            .detail
            .as_ref()
            .and_then(|d| d.body_text.clone())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// 正文 HTML 渲染接线
// ---------------------------------------------------------------------------

// 正文 HTML 文档缓存已迁入 DetailStore（`DetailStore::html_cache`，spec §7
// LRU 8 + T14 深色双通道）——DetailStore 是 AppShell 创建一次的长寿命实体，
// 跨详情视图重建存活，缓存随实体生灭，原先的进程级 static + Mutex 已删除。

/// 正文 HTML → 新渲染器元素 + 远程图 URL 清单；管线超限/净化后空
/// （`Err`）→ `None` 走纯文本降级。
///
/// `dark`：当前主题（T14 深色接线）——按当前主题取浅色原文或深色适配副本
/// （[`DetailStore::cached_doc`]，主题切换零重解析：两份产物都随缓存条目
/// 存活，深色副本 `OnceLock` 按需生成一次）。渲染层只消费适配后文档的字面
/// 颜色，不做二次反转（spec §6）。
///
/// 远程图清单（[`collect_remote_image_urls`]）供「加载图片」条的存在性
/// 判据（非空 ⇔ 存在 Remote 图）与放行后的逐图拉取清单。
fn html_body(
    store: &gpui_kit::Entity<DetailStore>,
    html: &str,
    email_id: i32,
    dark: bool,
    images: ImageCtx,
    cx: &mut gpui_kit::App,
) -> Option<(AnyElement, Vec<String>)> {
    let doc = store.update(cx, |store, _| store.cached_doc(email_id, html, dark))?;
    let remote_urls = collect_remote_image_urls(&doc);
    let el = render_html_document(&doc, dark, images, open_safe_url, cx);
    Some((el, remote_urls))
}

/// 链接打开动作（SafeUrl 之外不产生回调）：Http/Mailto 同走系统默认处理器
/// （`open::that_detached`——OAuth 授权链接 settings.rs 同款项目现有路径）。
fn open_safe_url(url: &postium_html::SafeUrl) {
    match url {
        postium_html::SafeUrl::Http(u) | postium_html::SafeUrl::Mailto(u) => {
            let _ = open::that_detached(u);
        }
    }
}

/// 纯文本正文块（现有纯文本路径；HTML 降级共用同一形态）。
fn plain_text_body(text: String) -> AnyElement {
    div()
        .flex_1()
        .overflow_hidden()
        .p_4()
        .child(
            div()
                .text_size(px(13.5))
                .line_height(relative(1.8))
                .child(text)
                .debug_selector(|| "email-plain-body".to_string()),
        )
        .into_any_element()
}

/// 未选中邮件的占位面板。
fn no_selection_placeholder(cx: &gpui_kit::App) -> AnyElement {
    v_flex()
        .flex_1()
        .size_full()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px(13.))
                .text_color(cx.theme().muted_foreground)
                .child(t("email.noEmailSelected")),
        )
        .into_any_element()
}

impl Render for EmailDetailView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ── 正文输入先行取齐并结束 store 借用：新渲染器需要 &mut App（读主题），
        // detail 共享借用不得跨越该调用 ──
        let body_input = {
            let store = self.store.read(cx);
            store
                .detail
                .as_ref()
                .map(|d| (d.email.id, store.html.clone().unwrap_or_default()))
        };
        let Some((email_id, body_html_str)) = body_input else {
            return no_selection_placeholder(cx);
        };

        // 当前主题（T14 深色接线）：dark=true 时渲染入口取深色适配副本
        // （DetailStore::cached_doc 双通道，spec §6；主题切换零重解析——
        // 解析与适配产物都随缓存条目存活）
        let dark = cx.theme().mode == ThemeMode::Dark;

        // 图片状态快照（Arc 共享 resolved + 克隆附件列表，借用即刻结束——
        // 渲染需要 &mut App；后台拉取写回 store 经 notify 触发本帧重渲染）
        let image_ctx = { self.store.read(cx).images.to_ctx() };
        let images_enabled = image_ctx.enabled;

        // 正文：纯文本邮件（html 空白，含 body_html 缺失 → DetailStore
        // html = Some("")）直走纯文本路径、无提示条——空 html 不进管线
        // （render_pipeline("") = Err(Empty) 会把纯文本邮件误标「无法渲染」，
        // 评审 P1）；HTML 邮件经新渲染器，管线超限/净化后空 → 纯文本降级 +
        // 顶部提示条
        let (body, html_fallback, remote_urls) = if body_html_str.trim().is_empty() {
            (
                plain_text_body(self.detail_body_text(cx)),
                false,
                Vec::new(),
            )
        } else {
            match html_body(&self.store, &body_html_str, email_id, dark, image_ctx, cx) {
                Some((el, urls)) => (
                    div()
                        .id("email-html-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_4()
                        .child(el)
                        .into_any_element(),
                    false,
                    urls,
                ),
                None => (plain_text_body(self.detail_body_text(cx)), true, Vec::new()),
            }
        };

        let store = self.store.read(cx);
        let Some(detail) = &store.detail else {
            return no_selection_placeholder(cx);
        };

        let sent_time = chrono::DateTime::from_timestamp(detail.email.sent_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let sender = detail
            .email
            .sender_name
            .clone()
            .unwrap_or_else(|| detail.email.sender_email.clone());

        v_flex()
            .flex_1()
            .size_full()
            .min_w(px(380.))
            .border_l_1()
            .border_color(cx.theme().border)
            // 头部：主题 + 操作
            .child(
                v_flex()
                    .p_3()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex_none()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .flex_1()
                                    .child(
                                        detail
                                            .email
                                            .subject
                                            .clone()
                                            .unwrap_or_else(|| "（无主题）".into()),
                                    ),
                            )
                            // 操作分组：主操作（回复/全部回复/转发）｜邮件态（星标/已读/重载）｜归置（归档/删除）｜浏览器
                            .child(
                                Button::new("reply")
                                    .primary()
                                    .xsmall()
                                    .label(t("email.reply"))
                                    .on_click(cx.listener(Self::reply)),
                            )
                            .child(
                                Button::new("reply-all")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.replyAll"))
                                    .on_click(cx.listener(Self::reply_all)),
                            )
                            .child(
                                Button::new("forward")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.forward"))
                                    .on_click(cx.listener(Self::forward)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("star")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Star)
                                    .when(detail.email.is_starred, |b| {
                                        b.label("★").tooltip(t("email.unstar"))
                                    })
                                    .when(!detail.email.is_starred, |b| b.tooltip(t("email.star")))
                                    .on_click(cx.listener(Self::toggle_star)),
                            )
                            .child(
                                Button::new("mark-unread")
                                    .ghost()
                                    .xsmall()
                                    .label(if detail.email.is_read {
                                        t("email.markUnread")
                                    } else {
                                        t("email.markRead")
                                    })
                                    .on_click(cx.listener(Self::mark_unread)),
                            )
                            .child(
                                Button::new("reload")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit_assets::IconName::RotateCw)
                                    .tooltip(t("email.reload"))
                                    .on_click(cx.listener(Self::reload)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("archive")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.archive"))
                                    .on_click(cx.listener(Self::archive)),
                            )
                            .child(
                                Button::new("delete")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.delete"))
                                    .on_click(cx.listener(Self::delete)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("browser")
                                    .ghost()
                                    .xsmall()
                                    .label("浏览器打开")
                                    .on_click(cx.listener(Self::open_in_browser)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("contact-link")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Contact)
                                    .label(t("contacts.link"))
                                    .on_click(cx.listener(Self::add_contact)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .text_size(px(12.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(sender)
                            .child("·")
                            .child(sent_time)
                            .child("·")
                            .child(t("email.to"))
                            .child(recipients_display(detail, self.recipients_expanded, cx))
                            .when_some(cc_display(detail), |this, cc| {
                                this.child("·").child(t("email.cc")).child(cc)
                            })
                            .when(recipient_count(detail) > 2, |this| {
                                this.child(
                                    Button::new("expand-recipients")
                                        .ghost()
                                        .xsmall()
                                        .label(if self.recipients_expanded {
                                            t("email.collapseRecipients").to_string()
                                        } else {
                                            t_format(
                                                "email.otherRecipients",
                                                &[(
                                                    "count",
                                                    &(recipient_count(detail) - 2).to_string(),
                                                )],
                                            )
                                            .to_string()
                                        })
                                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                            this.recipients_expanded = !this.recipients_expanded;
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
            // 附件条
            .when(!detail.attachments.is_empty(), |this| {
                this.child(
                    h_flex()
                        .flex_none()
                        .flex_wrap()
                        .gap_2()
                        .p_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .children(detail.attachments.iter().map(|att| {
                            let att_id = att.id;
                            let filename = att.filename.clone();
                            let api = self.api.clone();
                            let _ = api;
                            h_flex()
                                .gap_1()
                                .px_2()
                                .py_1()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(gpui_kit::component::Icon::new(IconName::Paperclip).size_3())
                                .child(div().text_size(px(12.)).child(format!("{}", att.filename)))
                                .child(
                                    Button::new(format!("open-att-{att_id}"))
                                        .ghost()
                                        .xsmall()
                                        .label(t("email.attachmentOpen"))
                                        .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                            this.open_attachment(att_id, e, w, cx)
                                        })),
                                )
                                .child(
                                    Button::new(format!("save-att-{att_id}"))
                                        .ghost()
                                        .xsmall()
                                        .label(t("email.attachmentSaveAs"))
                                        .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                            this.save_attachment_as(
                                                att_id,
                                                filename.clone(),
                                                e,
                                                w,
                                                cx,
                                            )
                                        })),
                                )
                        })),
                )
            })
            // 「加载图片」条：正文含远程图且未放行时显示（spec §4.4 隐私默认
            // 拦截）。整条可点击 → allow_remote_images（翻转放行态 + 逐图后台
            // 拉取）；放行后本条不再构建（重渲染消失）
            .children((!remote_urls.is_empty() && !images_enabled).then(|| {
                div()
                    .id("load-images")
                    .flex_none()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .bg(cx.theme().secondary)
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().accent.opacity(0.12)))
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(gpui_kit::component::Icon::new(IconName::Ban).size_3())
                    .child(t("email.loadImages"))
                    .on_click(cx.listener(Self::enable_remote_images))
                    .debug_selector(|| "email-load-images".to_string())
            }))
            // 降级提示条：HTML 无法渲染（超限/净化后空）时的纯文本降级标记
            .children(html_fallback.then(|| {
                div()
                    .flex_none()
                    .px_3()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("email.htmlFallback"))
                    .debug_selector(|| "email-html-fallback".to_string())
            }))
            // 正文
            .child(body)
            .into_any_element()
    }
}

fn recipient_count(detail: &postium_core::service::email_service::EmailDetail) -> usize {
    detail
        .recipient_emails
        .split([',', ';'])
        .filter(|s| !s.trim().is_empty())
        .count()
}

fn recipients_display(
    detail: &postium_core::service::email_service::EmailDetail,
    expanded: bool,
    cx: &gpui_kit::App,
) -> SharedString {
    let list: Vec<&str> = detail
        .recipient_emails
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let _ = cx;
    if expanded || list.len() <= 2 {
        list.join(", ").into()
    } else {
        format!("{}, {}", list[0], list[1]).into()
    }
}

fn cc_display(detail: &postium_core::service::email_service::EmailDetail) -> Option<SharedString> {
    detail
        .cc_emails
        .as_ref()
        .map(|cc| cc.trim())
        .filter(|cc| !cc.is_empty())
        .map(|cc| cc.into())
}

/// 工具条分组竖分隔线。
fn separator(cx: &gpui_kit::App) -> impl IntoElement {
    let _ = cx;
    div()
        .w(px(1.))
        .h(px(14.))
        .mx_1()
        .bg(gpui_kit::black().opacity(0.15))
}

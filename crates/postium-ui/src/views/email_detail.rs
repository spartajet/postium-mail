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
use postium_core::service::email_service::{ContactWrite, EmailDto};

use crate::i18n::{t, t_format};
use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;
use crate::stores::detail::DetailStore;
use crate::views::email_html::{ImageCtx, render_html_document};

// 正文划选动作（EmailHtmlBody key_context 内派发）：Ctrl+C 复制 / Esc 清选区。
gpui_kit::actions!([CopyHtmlSelection, ClearHtmlSelection]);

pub struct EmailDetailView {
    pub store: gpui_kit::Entity<DetailStore>,
    pub api: Arc<CoreApi>,
    /// 邮件列表弱引用：归档/删除/标未读后刷新列表（已删邮件不再留在列表）；
    /// 会话形态下另作时间线装载的竞态校验（selected_thread 仍为本线程）。
    pub emails: gpui_kit::WeakEntity<crate::stores::emails::EmailsStore>,
    pub recipients_expanded: bool,
    /// 正文划选状态（线上反馈①）：视图重建即重置（换邮件清选区），
    /// Ctrl+C 复制选中文本（html_body_copy 动作）。
    pub html_selection: crate::views::html_selection::HtmlSelection,
    /// 正文滚动区焦点（划选复制动作的派发锚点）。
    pub body_focus: gpui_kit::FocusHandle,
    /// 会话线程 id：`Some` = 会话时间线形态（Task 10），`None` = 既有
    /// 单封形态（平铺入口，行为零变化）。会话入口由宿主（app.rs 订阅
    /// `EmailsEvent::ThreadSelectionChanged`）重建本视图并在构造闭包内
    /// 立即调 [`Self::load_timeline`] 装载。
    pub thread_id: Option<i64>,
    /// 会话时间线摘要（`conversation_emails` 时间升序快照）。`None` =
    /// 非会话形态或时间线装载中（装载中渲染轻量 loading 态）。
    pub timeline: Option<Vec<EmailDto>>,
    /// 时间线内当前展开（渲染完整详情）的邮件 id：单展开态，点击折叠卡
    /// 切换；详情操作按钮读 store.detail，天然作用于展开封。
    pub expanded_id: Option<i32>,
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
        let timeline_mode = self.thread_id.is_some();
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(starred)) => {
                    store.update(cx, |s, cx| {
                        s.apply_star(starred);
                        cx.notify();
                    });
                    // 时间线形态：刷新快照（折叠卡无星标视觉，刷新落点在
                    // 列表行 starred_any 聚合 → 原位补丁，不重载列表）
                    if timeline_mode {
                        this.update(cx, |view, cx| view.refresh_timeline(false, None, cx))
                            .ok();
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "切换星标失败: {e}"),
                Err(_) => {}
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
        let timeline_mode = self.thread_id.is_some();
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(())) => {
                    if timeline_mode {
                        // 时间线形态：归档封仍在会话内（conversation_emails
                        // 仅除非草稿/未软删除，不按文件夹过滤），快照刷新
                        // 保持展开封与详情，列表聚合走保留选中的重载
                        this.update(cx, |view, cx| view.refresh_timeline(true, None, cx))
                            .ok();
                        return;
                    }
                    store.update(cx, |s, cx| {
                        s.detail = None;
                        s.html = None;
                        cx.notify();
                    });
                    // 详情清空后主窗口标题恢复默认（此路径不发 Loaded 事件）
                    restore_window_title(cx);
                    if let Some(list) = emails.upgrade() {
                        list.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                        });
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "归档邮件失败: {e}"),
                Err(_) => {}
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
        let deleted_id = detail.email.id;
        let rx = spawn_tokok_delete(api, deleted_id);
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        let timeline_mode = self.thread_id.is_some();
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(_)) => {
                    if timeline_mode {
                        // 时间线形态：被删封退出会话（removed_id 显式摘除），
                        // 快照刷新负责展开封相邻迁移/整组清空，列表聚合走
                        // 保留选中的重载
                        this.update(cx, |view, cx| {
                            view.refresh_timeline(true, Some(deleted_id), cx)
                        })
                        .ok();
                        return;
                    }
                    store.update(cx, |s, cx| {
                        s.detail = None;
                        s.html = None;
                        cx.notify();
                    });
                    // 详情清空后主窗口标题恢复默认（此路径不发 Loaded 事件）
                    restore_window_title(cx);
                    if let Some(list) = emails.upgrade() {
                        list.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                        });
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "删除邮件失败: {e}"),
                Err(_) => {}
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
        let message_id = detail.email.message_id.clone();
        let references = crate::views::compose::reply_references(
            detail.email.in_reply_to.as_deref(),
            detail.email.message_id.as_deref(),
        );
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::ReplyAll {
                    email_id: detail.email.id,
                    account_id: detail.email.account_id,
                    to,
                    subject,
                    original_html,
                    message_id,
                    references,
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
        let timeline_mode = self.thread_id.is_some();
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(())) => {
                    if timeline_mode {
                        // 时间线形态：会话是当前阅读面，不撤详情——快照刷新
                        // 更新列表行 unread 聚合（未读点回归）
                        this.update(cx, |view, cx| view.refresh_timeline(true, None, cx))
                            .ok();
                        return;
                    }
                    store.update(cx, |s, cx| {
                        s.detail = None;
                        s.html = None;
                        cx.notify();
                    });
                    // 详情清空后主窗口标题恢复默认（此路径不发 Loaded 事件）
                    restore_window_title(cx);
                    if let Some(list) = emails.upgrade() {
                        list.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                        });
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "标记未读失败: {e}"),
                Err(_) => {}
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
            match rx.recv().await {
                Ok(Ok(_)) => {
                    store.update(cx, |s, cx| {
                        s.force_load(id, &api_keep, cx);
                    });
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "从服务器重载邮件失败: {e}"),
                Err(_) => {}
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
        let message_id = detail.email.message_id.clone();
        let references = crate::views::compose::reply_references(
            detail.email.in_reply_to.as_deref(),
            detail.email.message_id.as_deref(),
        );
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::Reply {
                    email_id: detail.email.id,
                    account_id: detail.email.account_id,
                    to,
                    subject,
                    original_html,
                    message_id,
                    references,
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
        // review M-5：URL 清单随缓存条目（此前此处手动 parse + 全文档遍历）
        let urls = self.store.update(cx, |store, _| {
            store.remote_image_urls(email_id, &html)
        });
        if urls.is_empty() {
            return;
        }
        let api = self.api.clone();
        self.store
            .update(cx, |store, cx| store.allow_remote_images(urls, &api, cx));
    }

    /// 浏览器精确查看（table 布局保真兜底）。
    /// 划选复制（线上反馈①）：Ctrl+C 在正文滚动区焦点上派发，选中文本写
    /// 剪贴板并 toast 反馈；无选区时不动作（不清系统剪贴板）。
    fn copy_html_selection(&mut self, _: &CopyHtmlSelection, _: &mut Window, cx: &mut Context<Self>) {
        if !self.html_selection.is_selecting() {
            return; // 无选区不动作（不清系统剪贴板）
        }
        if let Some(text) = self.html_selection.selected_text() {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
            crate::notify::toast(t("email.selectionCopied"));
        }
    }

    /// Esc 清空划选（浏览器一致语义）。
    fn clear_html_selection(&mut self, _: &ClearHtmlSelection, _: &mut Window, cx: &mut Context<Self>) {
        self.html_selection.clear();
        cx.notify();
    }

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

    /// 会话时间线装载（宿主 `ThreadSelectionChanged` → 视图构造后调用）：
    /// `conversation_emails` 填 [`Self::timeline`]（时间升序）→ 默认展开
    /// 最后一封（sent_at 最大，复用 `DetailStore::load` 装载完整详情，
    /// 单封渲染全链路原样复用）→ 会话含未读时 `mark_conversation_read`
    /// 清零。
    ///
    /// 竞态防护（对齐 [`DetailStore::load_impl`] 的「返回时校验」模式）：
    /// 装载是异步的，返回时用户可能已切走——切邮件/切会话/切档/导航都会
    /// 改写 `EmailsStore::selected_thread` 并由宿主重建本视图，完成回调
    /// 先校验列表选中仍是本线程，过期结果整体丢弃（不装载详情、不清未读）。
    pub(crate) fn load_timeline(&mut self, thread_id: i64, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.conversation_emails(thread_id).await });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(rows)) = rx.recv().await else {
                // 装载失败：保持 loading 占位（与列表装载失败口径一致）
                tracing::warn!(target: "postium::ui", thread_id, "会话时间线加载失败");
                return;
            };
            this.update(cx, |view, cx| {
                // 过期结果：用户已切走（选中不再是本线程）——整体丢弃
                let still_selected = view
                    .emails
                    .upgrade()
                    .is_some_and(|list| list.read(cx).selected_thread == Some(thread_id));
                if !still_selected {
                    return;
                }
                view.timeline = Some(rows.clone());
                // 默认展开最后一封（时间线升序 ⇒ sent_at 最大；max_by_key
                // 对上游排序契约防御）
                if let Some(last) = rows.iter().max_by_key(|e| e.sent_at) {
                    let id = last.id;
                    view.expanded_id = Some(id);
                    let api = view.api.clone();
                    view.store.update(cx, |s, cx| s.load(id, &api, cx));
                }
                cx.notify();
                // 会话含未读 → 批量标已读（未读清零）；全读会话不重复触发
                if rows.iter().any(|e| !e.is_read) {
                    view.mark_timeline_read(thread_id, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// 会话级标已读 + 列表/侧栏刷新：对齐既有标已读路径（email_list 行
    /// 点击标已读）——列表侧本地补丁行数据（会话行 unread 归零 +
    /// list_version 原位刷新，不重置滚动/选中），侧栏徽章走
    /// `EmailsEvent::UnreadChanged`（宿主重载文件夹统计）。
    fn mark_timeline_read(&mut self, thread_id: i64, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let emails = self.emails.clone();
        let rx = spawn_tokio(async move { api.mark_conversation_read(thread_id).await });
        cx.spawn(async move |_, cx| {
            if !matches!(rx.recv().await, Ok(Ok(()))) {
                return;
            }
            let Some(list) = emails.upgrade() else { return };
            list.update(cx, |s, cx| {
                // 本地清会话行未读：时间线快照与列表行各自持有 DTO 副本，
                // store 侧补丁 + list_version 令宿主原位刷新行数据
                if let Some(conv) = s
                    .conversations
                    .iter_mut()
                    .find(|c| c.thread_id == thread_id)
                {
                    conv.unread = 0;
                    s.list_version += 1;
                }
                // 侧栏未读徽章（既有标已读同路径）
                cx.emit(crate::stores::emails::EmailsEvent::UnreadChanged);
                cx.notify();
            });
        })
        .detach();
    }

    /// 时间线操作后刷新（T10 承接 I-1）：重拉 `conversation_emails` 快照
    /// 应用到 [`Self::timeline`]（被删封移出、展开封相邻迁移等由
    /// [`Self::apply_timeline_snapshot`] 处理）。
    ///
    /// `list_reload`：`true` = 列表聚合可能变化（封数/预览/未读/线程
    /// 存续）→ `refresh_now_preserving_selection` 重载会话档（**保留会话
    /// 选中**——本刷新完成回调的竞态校验依赖「列表选中仍是本线程」，
    /// 普通 refresh_now 会清选中导致后续刷新被误判过期）；`false` = 仅
    /// 星标变化 → 原位补丁该行 `starred_any`（T9 `mark_timeline_read`
    /// 同路径，不动滚动/选中）。
    ///
    /// `removed_id`：本次操作**删除**的邮件 id——第一阶段删除 = 移入
    /// Trash（文件夹移动，非软删），`conversation_emails` 不按文件夹过滤
    /// 仍会返回它；时间线语义上被删封应退出会话，应用快照前显式摘除
    /// （归档不移除——归档封保留在线程中，对齐会话语义）。
    ///
    /// 竞态防护与 [`Self::load_timeline`] 同口径：完成回调校验列表选中
    /// 仍是本线程，过期结果整体丢弃。
    pub(crate) fn refresh_timeline(
        &mut self,
        list_reload: bool,
        removed_id: Option<i32>,
        cx: &mut Context<Self>,
    ) {
        let Some(thread_id) = self.thread_id else { return; };
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.conversation_emails(thread_id).await });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(rows)) = rx.recv().await else {
                tracing::warn!(target: "postium::ui", thread_id, "会话时间线刷新失败");
                return;
            };
            this.update(cx, |view, cx| {
                let still_selected = view
                    .emails
                    .upgrade()
                    .is_some_and(|list| list.read(cx).selected_thread == Some(thread_id));
                if !still_selected {
                    return; // 过期结果：用户已切走
                }
                view.apply_timeline_snapshot(thread_id, rows, list_reload, removed_id, cx);
            })
            .ok();
        })
        .detach();
    }

    /// 应用新时间线快照：更新 [`Self::timeline`] + 展开封迁移 + 列表刷新。
    ///
    /// 展开封迁移（被删封是展开封时）：移到相邻封——优先**下一封**
    /// （时间更晚 = 旧快照中被删位右侧，新快照同索引），无则上一封；
    /// 并 `DetailStore::load` 装载新展开封。归档封仍在会话内
    /// （`conversation_emails` 不按文件夹过滤），不触发迁移。
    ///
    /// 会话整组归置（快照为空）：清详情 + 清展开态，主窗口标题恢复默认，
    /// 列表常规重载（`refresh_now`，选中随之清除——线程已不存在）。
    fn apply_timeline_snapshot(
        &mut self,
        thread_id: i64,
        rows: Vec<EmailDto>,
        list_reload: bool,
        removed_id: Option<i32>,
        cx: &mut Context<Self>,
    ) {
        // 被删封显式摘除（删除 = 移 Trash，文件夹无关的会话查询仍返回它）
        let rows = match removed_id {
            Some(removed) if rows.iter().any(|e| e.id == removed) => rows
                .into_iter()
                .filter(|e| e.id != removed)
                .collect::<Vec<_>>(),
            _ => rows,
        };
        if rows.is_empty() {
            self.timeline = Some(Vec::new());
            self.expanded_id = None;
            self.store.update(cx, |s, cx| {
                s.detail = None;
                s.html = None;
                cx.notify();
            });
            restore_window_title_sync(cx);
            if let Some(list) = self.emails.upgrade() {
                let api = self.api.clone();
                list.update(cx, |s, cx| {
                    s.refresh_now(&api, cx);
                });
            }
            cx.notify();
            return;
        }

        let expanded_gone = self.expanded_id.is_some_and(|id| !rows.iter().any(|e| e.id == id));
        if expanded_gone {
            // 相邻封迁移：优先下一封（更晚），无则上一封（规则见纯函数）
            let old = self.timeline.clone().unwrap_or_default();
            let neighbor = self
                .expanded_id
                .and_then(|id| neighbor_after_removal(&old, id, &rows));
            if let Some(id) = neighbor {
                self.expanded_id = Some(id);
                let api = self.api.clone();
                self.store.update(cx, |s, cx| s.load(id, &api, cx));
            }
        } else if self.store.read(cx).detail.is_none() {
            // 展开封仍在会话但详情被操作清空：重载展开封
            if let Some(id) = self.expanded_id {
                let api = self.api.clone();
                self.store.update(cx, |s, cx| s.load(id, &api, cx));
            }
        }
        let starred_any = rows.iter().any(|e| e.is_starred);
        self.timeline = Some(rows);
        cx.notify();

        // 列表侧刷新（会话行聚合随快照真值收敛）
        if let Some(list) = self.emails.upgrade() {
            let api = self.api.clone();
            if list_reload {
                list.update(cx, |s, cx| {
                    s.refresh_now_preserving_selection(&api, cx);
                });
            } else {
                // 仅星标变化：原位补丁 starred_any（聚合真值取自新快照）
                list.update(cx, |s, cx| {
                    if let Some(conv) = s
                        .conversations
                        .iter_mut()
                        .find(|c| c.thread_id == thread_id)
                    {
                        conv.starred_any = starred_any;
                        s.list_version += 1;
                        cx.notify();
                    }
                });
            }
        }
    }

    /// 折叠卡点击：切换展开封（单展开态）——展开该封复用
    /// `DetailStore::load` 装载完整详情，原展开封折叠回收为卡。详情操作
    /// 按钮（回复/归档/删除等）读 store.detail，天然作用于新展开封。
    fn expand_timeline_email(
        &mut self,
        email_id: i32,
        _: &ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.expanded_id == Some(email_id) {
            return; // 已是展开封：store 已装载，不重复装载
        }
        self.expanded_id = Some(email_id);
        // 换封清划选区（对齐平铺形态「视图重建即重置」的换邮件语义）
        self.html_selection.clear();
        let api = self.api.clone();
        self.store.update(cx, |s, cx| s.load(email_id, &api, cx));
        cx.notify();
    }
}

fn spawn_tokok_delete(
    api: Arc<CoreApi>,
    id: i32,
) -> smol::channel::Receiver<Result<usize, postium_core::error::MailError>> {
    spawn_tokio(async move { api.delete_emails(vec![id]).await })
}

/// 供 e2e 驱动详情操作按钮的测试钩子（工具条按钮 on_click 同款 handler
/// 链——按钮本体无探针，坐标点击脆弱；与 email_list::test_hooks 同款
/// 口径，作用于当前展开封/当前详情）。
#[cfg(test)]
pub(crate) mod test_hooks {
    use super::*;

    /// 触发详情工具条「删除」按钮同款路径（时间线形态走
    /// `refresh_timeline(removed_id)` 相邻迁移链路）。
    pub(crate) fn delete_for_test(
        view: &gpui_kit::Entity<EmailDetailView>,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) {
        view.update(cx, |view, cx| {
            view.delete(&ClickEvent::default(), window, cx);
        });
    }
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
///
/// panic 策略：无 catch_unwind，依赖无 panic 向量构造 + GPUI 全局 hook
/// （终审 I-1 裁定）。
fn html_body(
    store: &gpui_kit::Entity<DetailStore>,
    html: &str,
    email_id: i32,
    dark: bool,
    images: ImageCtx,
    selection: crate::views::html_selection::HtmlSelection,
    cx: &mut gpui_kit::App,
) -> Option<(AnyElement, Vec<String>)> {
    let doc = store.update(cx, |store, _| store.cached_doc(email_id, html, dark))?;
    // review M-5：URL 清单随缓存条目一次计算（此前每帧全文档遍历）
    let remote_urls =
        store.update(cx, |store, _| store.remote_image_urls(email_id, html));
    let el = render_html_document(&doc, dark, images, selection, open_safe_url, cx);
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

/// 详情面板外框（平铺/会话时间线两形态共用）：左侧分隔线 + 最小宽 +
/// 高度链。显式 min_h_0：正文滚动区之上最后一级列容器——HTML 正文超
/// 视口时 min-content 高度沿祖先链上泄（滚动区的 min_h_0 只封自己），
/// 把主内容列撑满整窗、底部状态栏挤出窗口（线上反馈：显示邮件时
/// 状态栏消失）。
fn pane_chrome(inner: AnyElement, cx: &gpui_kit::App) -> AnyElement {
    v_flex()
        .flex_1()
        .size_full()
        .min_h_0()
        .min_w(px(380.))
        .border_l_1()
        .border_color(cx.theme().border)
        .child(inner)
        .into_any_element()
}

/// 会话时间线装载中的整面板占位（点会话行后、时间线到达前——右侧不停
/// 留旧邮件，T9 承接①）。
fn loading_placeholder(cx: &gpui_kit::App) -> AnyElement {
    pane_chrome(
        v_flex()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("email.loading")),
            )
            .into_any_element(),
        cx,
    )
}

/// 展开位在详情装载完成前的轻量占位（会话时间线形态内嵌）。
fn timeline_slot_placeholder(loading: bool, cx: &gpui_kit::App) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(13.))
        .text_color(cx.theme().muted_foreground)
        .child(if loading {
            t("email.loading")
        } else {
            t("email.noEmailSelected")
        })
        .into_any_element()
}

/// 详情面板被操作清空（归档/删除/标未读）后恢复主窗口默认标题。
/// 这些路径不经过 DetailStore::load，不会发 DetailEvent::Loaded。
fn restore_window_title(cx: &mut gpui_kit::AsyncApp) {
    let _ = cx.update(|app| {
        if let Some(main) = app.try_global::<crate::app::MainShell>()
            && let Some(shell) = main.0.upgrade()
        {
            let _ = shell.update(app, |shell, cx| shell.update_window_title(cx));
        }
    });
}

/// [`restore_window_title`] 的同步形态（时间线快照应用路径在
/// `Context<Self>` 内直调，无 AsyncApp 可借）。
fn restore_window_title_sync(cx: &mut gpui_kit::App) {
    if let Some(main) = cx.try_global::<crate::app::MainShell>()
        && let Some(shell) = main.0.upgrade()
    {
        let _ = shell.update(cx, |shell, cx| shell.update_window_title(cx));
    }
}

impl EmailDetailView {
    /// 无展开详情时的面板分流：会话形态渲染卡列 + 展开位占位；时间线
    /// 装载中显示 loading（T9 承接①：点会话行后右侧不停留旧邮件）；
    /// 已装载但为空（会话整组归置后的快照刷新）回落无选中占位——
    /// 线程已从列表消失，不再停留 loading；平铺形态维持无选中占位
    /// （既有行为零变化）。
    fn render_without_detail(&self, detail_loading: bool, cx: &mut Context<Self>) -> AnyElement {
        if self.timeline.as_ref().is_some_and(|rows| !rows.is_empty()) {
            return self.render_timeline(None, detail_loading, cx);
        }
        if self.thread_id.is_some() {
            return if self.timeline.is_some() {
                no_selection_placeholder(cx)
            } else {
                loading_placeholder(cx)
            };
        }
        no_selection_placeholder(cx)
    }

    /// 会话时间线形态渲染：折叠卡列（时间升序）+ 展开封按时间序占位嵌入
    /// 完整单封详情（既有渲染原样复用）。卡列各自限高滚动，展开详情占
    /// 余高——正文滚动/焦点/划选语义与单封形态完全一致。
    fn render_timeline(
        &self,
        detail_el: Option<AnyElement>,
        detail_loading: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // 调用方保证时间线非空（防御回落）
        let Some(rows) = self.timeline.as_ref().filter(|rows| !rows.is_empty()) else {
            return no_selection_placeholder(cx);
        };
        let (pre, post) = match rows.iter().position(|e| Some(e.id) == self.expanded_id) {
            // 展开封按时间序占位：前段卡列在上、详情居中、后段卡列在下
            Some(i) => (&rows[..i], &rows[i + 1..]),
            // 展开封不在时间线（如已归置、快照刷新前）：全卡列 + 详情位
            None => (&rows[..], &[] as &[EmailDto]),
        };
        let col = v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .when(!pre.is_empty(), |this| {
                this.child(Self::timeline_cards(pre, true, cx))
            })
            .child(detail_el.unwrap_or_else(|| timeline_slot_placeholder(detail_loading, cx)))
            .when(!post.is_empty(), |this| {
                this.child(Self::timeline_cards(post, false, cx))
            });
        pane_chrome(col.into_any_element(), cx)
    }

    /// 折叠卡列：发件人 + 日期（复用列表时间格式化 helper）+ 预览首行；
    /// 点击切换展开封。e2e 探针：`thread-card-{email_id}`。
    ///
    /// `above_detail`：卡列位于展开详情上方（border-b 分隔）或下方
    /// （border-t 分隔）。
    fn timeline_cards(rows: &[EmailDto], above_detail: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut pane = v_flex()
            // 滚动容器需有状态（overflow_y_scroll 在 Stateful 元素上）
            .id(if above_detail {
                "thread-cards-pre"
            } else {
                "thread-cards-post"
            })
            .flex_none()
            .max_h(px(220.))
            .overflow_y_scroll()
            .children(rows.iter().map(|email| {
                let id = email.id;
                let time = crate::views::email_list::format_list_time(email.sent_at);
                let sender = email
                    .sender_name
                    .clone()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| email.sender_email.clone());
                let preview = email.preview.clone().unwrap_or_default();
                h_flex()
                    .id(SharedString::from(format!("thread-card-{id}")))
                    .w_full()
                    .flex_none()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().accent.opacity(0.08)))
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(180.))
                            .overflow_hidden()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .whitespace_nowrap()
                            .child(sender),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(11.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(time),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_nowrap()
                            .line_clamp(1)
                            .child(preview),
                    )
                    .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                        this.expand_timeline_email(id, e, w, cx);
                    }))
                    // e2e 探针（debug 构建记录 bounds，正式构建空操作）
                    .debug_selector(move || format!("thread-card-{id}"))
            }));
        if above_detail {
            pane = pane.border_b_1();
        } else {
            pane = pane.border_t_1();
        }
        pane.border_color(cx.theme().border).into_any_element()
    }
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
        let detail_loading = self.store.read(cx).loading;
        let Some((email_id, body_html_str)) = body_input else {
            return self.render_without_detail(detail_loading, cx);
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
            match html_body(
                &self.store,
                &body_html_str,
                email_id,
                dark,
                image_ctx,
                self.html_selection.clone(),
                cx,
            ) {
                Some((el, urls)) => (
                    div()
                        .id("email-html-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_4()
                        // 划选复制（线上反馈①）：正文滚动区可聚焦（点击文本即
                        // 焦点），Ctrl+C 在 key_context 内派发 copy_html_selection
                        .track_focus(&self.body_focus)
                        .key_context("EmailHtmlBody")
                        .on_action(cx.listener(Self::copy_html_selection))
                        .on_action(cx.listener(Self::clear_html_selection))
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
            // 装载输入读取与详情读取之间被并发清空（同帧理论不可达）：
            // 与无详情分流同口径
            return self.render_without_detail(detail_loading, cx);
        };

        let sent_time = chrono::DateTime::from_timestamp(detail.email.sent_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let sender = detail
            .email
            .sender_name
            .clone()
            .unwrap_or_else(|| detail.email.sender_email.clone());

        let detail_content = v_flex()
            .flex_1()
            .min_h_0()
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
            .child(body);

        // 面板组装：会话时间线形态把完整详情嵌入展开位（上下卡列按时间序
        // 排布）；平铺形态（timeline 置空）详情本体即面板，既有行为零变化
        if self.timeline.as_ref().is_some_and(|rows| !rows.is_empty()) {
            self.render_timeline(Some(detail_content.into_any_element()), detail_loading, cx)
        } else {
            pane_chrome(detail_content.into_any_element(), cx)
        }
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

/// 展开封被移出会话后的相邻封迁移目标（纯函数，Task 11；单测见
/// e2e_tests.rs——本文件 GPUI builder 链过深，内联 `#[test]` 会触顶
/// 宏展开递归上限）。
///
/// 旧快照 `old_rows` 与新快照 `new_rows` 均时间升序；`removed_id` 在
/// 新快照中已不存在（软删/归置）。规则：优先**下一封**（时间更晚 =
/// 新快照同索引位——删除使左移一位，原右侧那封补到被删位），无则
/// **上一封**（被删的是最后一封），旧快照定位失败（防御）回落新快照
/// 最后一封；新快照为空返回 `None`（整组归置，调用方另行处理）。
pub(crate) fn neighbor_after_removal(
    old_rows: &[EmailDto],
    removed_id: i32,
    new_rows: &[EmailDto],
) -> Option<i32> {
    old_rows
        .iter()
        .position(|e| e.id == removed_id)
        .and_then(|pos| {
            new_rows
                .get(pos)
                .map(|e| e.id)
                .or_else(|| new_rows.get(pos.saturating_sub(1)).map(|e| e.id))
        })
        .or_else(|| new_rows.last().map(|e| e.id))
}

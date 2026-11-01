//! 邮件列表视图（移植自 src/lib/components/email/EmailList.svelte，
//! 虚拟化模式参照 spike 已验证实现）。
//!
//! 双行模型（Task 9）：列表行有两种形态——平铺档单封邮件行
//! ([`EmailRow`]，现状不变)与会话档线程聚合行([`ConversationRow`])。
//! 行数据源由 [`EmailsStore::in_conversation_view`] 决定，宿主(app.rs)
//! 据此在 `set_rows`/`set_conversations` 间分派；列表头部工具条提供
//! 会话/平铺二态切换按钮(`set_view_mode`)。
//!
//! 交互约定：行选中 = 打开该邮件（写 `EmailsStore.selected` 并异步标已读），
//! 宿主订阅 store 渲染详情；会话行选中 = `EmailsStore::select_thread`
//! （发 `ThreadSelectionChanged`，会话时间线由 T10 接线）。

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    IndexPath,
    Sizable as _,
    Selectable,
    button::{Button, ButtonVariants as _},
    h_flex,
    list::{List, ListDelegate, ListItem, ListState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::Disableable as _;
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::{ConversationSummary, EmailCategory, EmailDto};

use crate::i18n::t;
use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;
use crate::stores::emails::{EmailsStore, EmailViewMode};

/// 列表行时间列格式化（平铺行与会话行共用，勿在行组件内复制逻辑）：
/// 当天显示 `HH:MM`，否则 `MM-DD`。
/// Task 10 起会话时间线折叠卡（email_detail）亦复用（pub(crate)）。
pub(crate) fn format_list_time(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| {
            let now = chrono::Utc::now();
            if dt.date_naive() == now.date_naive() {
                dt.format("%H:%M").to_string()
            } else {
                dt.format("%m-%d").to_string()
            }
        })
        .unwrap_or_default()
}

#[derive(IntoElement)]
pub struct EmailRow {
    base: ListItem,
    email: Rc<EmailDto>,
    selected: bool,
    /// 右键菜单所需：核心 API 与列表 store（弱引用）
    api: Option<Arc<CoreApi>>,
    store: Option<gpui_kit::WeakEntity<EmailsStore>>,
}

impl Selectable for EmailRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.base = self.base.selected(selected);
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for EmailRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let menu_ctx = (self.api.clone(), self.store.clone());
        let email_rc = self.email.clone();
        let time = format_list_time(self.email.sent_at);
        let unread = !self.email.is_read;

        // 三行等高结构（对齐 Tauri 版 EmailList.svelte 的邮件项）：
        // ①发件人+时间 ②星标+主题 ③预览（单行，空时占位保持行高一致）。
        // 每行容器固定高度，行与行之间 border-b 分隔。
        let line = |h: f32| div().w_full().h(px(h)).overflow_hidden();

        // ① 发件人 + 时间；未读时发件人加粗（tauri: font-semibold text-foreground）
        let mut row1 = h_flex().gap_2().items_center();
        if unread {
            row1 = row1.child(
                div()
                    .flex_none()
                    .w(px(6.))
                    .h(px(6.))
                    .rounded_full()
                    .bg(cx.theme().accent),
            );
        }
        row1 = row1
            .w_full()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.5))
                    .when(unread, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().foreground))
                    .when(!unread, |d| d.text_color(cx.theme().muted_foreground))
                    .whitespace_nowrap()
                    .line_clamp(1)
                    .child(
                        self.email
                            .sender_name
                            .clone()
                            .unwrap_or_else(|| self.email.sender_email.clone()),
                    ),
            )
            .when(self.email.has_attachments, |this| {
                this.child(
                    gpui_kit::component::Icon::new(IconName::Paperclip)
                        .size_3()
                        .flex_none()
                        .text_color(cx.theme().muted_foreground),
                )
            })
            .child(
                div()
                    .flex_none()
                    .text_size(px(11.5))
                    .text_color(cx.theme().muted_foreground)
                    .child(time),
            );

        // ② 星标 + 主题；未读时主题中粗
        let row2 = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .when(self.email.is_starred, |this| {
                this.child(
                    gpui_kit::component::Icon::new(IconName::Star)
                        .size_3()
                        .flex_none()
                        .text_color(gpui_kit::rgb(0xf59e0b)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.5))
                    .when(unread, |d| d.font_weight(FontWeight::MEDIUM).text_color(cx.theme().foreground))
                    .when(!unread, |d| d.text_color(cx.theme().muted_foreground))
                    .whitespace_nowrap()
                    .line_clamp(1)
                    .child(
                        self.email
                            .subject
                            .clone()
                            .unwrap_or_else(|| "（无主题）".into()),
                    ),
            );

        // ③ 预览单行；空时占位同高，保证全部邮件等高
        let preview_text = self.email.preview.clone().unwrap_or_default();
        let row3 = div()
            .mt(px(2.))
            .h(px(15.))
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(cx.theme().muted_foreground)
            .when(!preview_text.is_empty(), |d| {
                d.whitespace_nowrap().line_clamp(1).child(preview_text)
            });

        let row = self
            .base
            .px(px(20.))
            .py(px(12.))
            // 行间分隔线（全强度主题分隔色；最后一行也带，与“加载更多”区隔开）
            .border_b_1()
            .border_color(cx.theme().border)
            .child(line(18.).child(row1))
            .child(line(18.).mt(px(2.)).child(row2))
            .child(row3);

        // 右键菜单（对齐旧版 EmailContextMenu；旧版”更多操作”为占位未复刻）
        match menu_ctx {
            (Some(api), Some(store)) => div()
                .child(row)
                .context_menu(move |menu, _, _| {
                    build_row_menu(menu, email_rc.clone(), api.clone(), store.clone())
                })
                .into_any_element(),
            _ => row.into_any_element(),
        }
    }
}

/// 会话（线程聚合）行：对齐 [`EmailRow`] 的三行视觉约定（字号/间距/
/// 选中态/未读加粗/星标图标/时间格式化均复用同一套）。
///
/// 与单封行的差异：无附件图标（`ConversationSummary` 不携带附件信息）；
/// 第一行为**参与者摘要**（spec §5：去重对方邮箱、排除本账号地址、超过
/// 3 个示 +N——最后发件人已在摘要首位，见 [`participants_summary_text`]；
/// 空摘要回退线程最后发件人）+ 封数角标（`total > 1`），第二行为主题，
/// 第三行为最新邮件预览。e2e 探针：`conv-row-{thread_id}`。
///
/// 会话级操作（Task 11）：星标图标常驻可点（stop_propagation 防误触
/// 行选中 → [`act_on_conversation`] 星标档）；右键菜单对齐平铺行形态
/// （星标切换 ─ 删除会话）。
#[derive(IntoElement)]
pub struct ConversationRow {
    base: ListItem,
    conversation: Rc<ConversationSummary>,
    selected: bool,
    /// 右键菜单与星标点击所需：核心 API 与列表 store（弱引用）
    api: Option<Arc<CoreApi>>,
    store: Option<gpui_kit::WeakEntity<EmailsStore>>,
}

/// 会话行参与者摘要文本（spec §5，i18n 无新键——+N 为数字拼接）：
/// 前 3 个邮箱以「, 」连接，剩余以「 +N」收尾（N=0 省略计数段）；
/// 空列表返回 None（行内回退既有最后发件人展示）。
/// `participants` 已由聚合查询去重、排除本账号地址，且最后发件人置首。
fn participants_summary_text(participants: &[String]) -> Option<String> {
    if participants.is_empty() {
        return None;
    }
    let shown = participants
        .iter()
        .take(3)
        .map(String::as_str)
        .collect::<Vec<_>>();
    let rest = participants.len() - shown.len();
    Some(if rest > 0 {
        format!("{} +{rest}", shown.join(", "))
    } else {
        shown.join(", ")
    })
}

impl Selectable for ConversationRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.base = self.base.selected(selected);
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ConversationRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let conv_rc = self.conversation.clone();
        let conv = &self.conversation;
        let time = format_list_time(conv.received_at);
        // 未读语义按会话聚合数（unread > 0）而非单封 is_read
        let unread = conv.unread > 0;

        // 三行等高结构（与 EmailRow 同构，行高/内距/分隔线一致）
        let line = |h: f32| div().w_full().h(px(h)).overflow_hidden();

        // ① 参与者摘要（最后发件人置首；空摘要回退最后发件人）+ 封数角标
        //    + 时间；未读加粗 + 未读点
        let mut row1 = h_flex().gap_2().items_center();
        if unread {
            row1 = row1.child(
                div()
                    .flex_none()
                    .w(px(6.))
                    .h(px(6.))
                    .rounded_full()
                    .bg(cx.theme().accent),
            );
        }
        // ① 发件人槽 = 参与者摘要（最后发件人已置首，样式沿用既有 sender
        // 渲染：字号/未读加粗同槽）；空摘要回退既有「发件人名/邮箱」展示
        let sender_text = participants_summary_text(&conv.participants).unwrap_or_else(|| {
            conv.sender_name
                .clone()
                .unwrap_or_else(|| conv.sender_email.clone())
        });
        row1 = row1
            .w_full()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.5))
                    .when(unread, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().foreground))
                    .when(!unread, |d| d.text_color(cx.theme().muted_foreground))
                    .whitespace_nowrap()
                    .line_clamp(1)
                    .child(sender_text),
            );
        // 封数角标：多封会话显示 total（形态对齐侧栏未读徽标）
        if conv.total > 1 {
            row1 = row1.child(
                div()
                    .flex_none()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(8.))
                    .text_size(px(10.5))
                    .bg(cx.theme().secondary)
                    .text_color(cx.theme().secondary_foreground)
                    .child(conv.total.to_string()),
            );
        }
        row1 = row1.child(
            div()
                .flex_none()
                .text_size(px(11.5))
                .text_color(cx.theme().muted_foreground)
                .child(time),
        );

        // ② 星标（可点，常驻显示：已星标金色 / 未星标淡化，点击切换会话
        // 星标——stop_propagation 防误触行选中）+ 主题；未读中粗
        let thread_for_star = conv.thread_id;
        let starred_any = conv.starred_any;
        let star_clickable = match (&self.api, &self.store) {
            (Some(api), Some(store)) => Some((api.clone(), store.clone())),
            _ => None,
        };
        let star_icon = gpui_kit::component::Icon::new(IconName::Star)
            .size_3()
            .flex_none()
            .text_color(if starred_any {
                gpui_kit::rgb(0xf59e0b).into()
            } else {
                cx.theme().muted_foreground.opacity(0.45)
            });
        let star_slot = match star_clickable {
            Some((api, store)) => div()
                .id("conv-star")
                .flex_none()
                .cursor_pointer()
                .child(star_icon)
                .on_click(move |_, _, cx| {
                    // 防误触行选中（星标点击不打开会话时间线）
                    cx.stop_propagation();
                    act_on_conversation(
                        api.clone(),
                        store.clone(),
                        thread_for_star,
                        ConvAction::ToggleStar,
                        cx,
                    );
                })
                .into_any_element(),
            // 无操作上下文（非列表宿主）：退化为纯指示（仅已星标显示）
            None => starred_any
                .then(|| star_icon.into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
        };
        let row2 = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(star_slot)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.5))
                    .when(unread, |d| d.font_weight(FontWeight::MEDIUM).text_color(cx.theme().foreground))
                    .when(!unread, |d| d.text_color(cx.theme().muted_foreground))
                    .whitespace_nowrap()
                    .line_clamp(1)
                    .child(
                        conv.subject
                            .clone()
                            .unwrap_or_else(|| "（无主题）".into()),
                    ),
            );

        // ③ 最新邮件预览单行；空时占位同高
        let preview_text = conv.preview.clone().unwrap_or_default();
        let row3 = div()
            .mt(px(2.))
            .h(px(15.))
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(cx.theme().muted_foreground)
            .when(!preview_text.is_empty(), |d| {
                d.whitespace_nowrap().line_clamp(1).child(preview_text)
            });

        let thread_id = conv.thread_id;
        let row = self
            .base
            .px(px(20.))
            .py(px(12.))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(line(18.).child(row1))
            .child(line(18.).mt(px(2.)).child(row2))
            .child(row3)
            // e2e 探针（debug 构建记录 bounds，正式构建空操作）
            .debug_selector(move || format!("conv-row-{thread_id}"));

        // 右键菜单（对齐平铺行菜单形态；单封语义操作——回复/转发/已读
        // ——不适用于会话级，只保留星标切换与删除会话）
        let menu_ctx = (self.api.clone(), self.store.clone());
        match menu_ctx {
            (Some(api), Some(store)) => div()
                .child(row)
                .context_menu(move |menu, _, _| {
                    build_conv_menu(menu, conv_rc.clone(), api.clone(), store.clone())
                })
                .into_any_element(),
            _ => row.into_any_element(),
        }
    }
}

/// 列表行统一元素形态：`ListDelegate::Item` 须为单一类型且实现
/// `Selectable + IntoElement`，故以枚举分发到平铺/会话两种行组件。
#[derive(IntoElement)]
pub enum EmailListRow {
    Email(EmailRow),
    Conversation(ConversationRow),
}

impl Selectable for EmailListRow {
    fn selected(self, selected: bool) -> Self {
        match self {
            Self::Email(row) => Self::Email(row.selected(selected)),
            Self::Conversation(row) => Self::Conversation(row.selected(selected)),
        }
    }
    fn is_selected(&self) -> bool {
        match self {
            Self::Email(row) => row.is_selected(),
            Self::Conversation(row) => row.is_selected(),
        }
    }
}

impl RenderOnce for EmailListRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            // 显式消歧：View trait 也有 render（&mut self 形态）
            Self::Email(row) => RenderOnce::render(row, window, cx).into_any_element(),
            Self::Conversation(row) => RenderOnce::render(row, window, cx).into_any_element(),
        }
    }
}

/// 行级操作类型。
enum RowAction {
    ToggleStar,
    MarkRead(bool),
    Delete,
}

/// 对指定邮件执行异步操作，完成后刷新列表 store。
fn act_on_email(
    api: Arc<CoreApi>,
    store: gpui_kit::WeakEntity<EmailsStore>,
    id: i32,
    action: RowAction,
    cx: &mut App,
) {
    let api_keep = api.clone();
    let action_label = match action {
        RowAction::ToggleStar => "ToggleStar",
        RowAction::MarkRead(_) => "MarkRead",
        RowAction::Delete => "Delete",
    };
    let rx = spawn_tokio(async move {
        match action {
            RowAction::ToggleStar => api.toggle_star(id).await.map(|_| ()),
            RowAction::MarkRead(read) => api.mark_as_read(id, read).await,
            RowAction::Delete => api.delete_emails(vec![id]).await.map(|_| ()),
        }
    });
    let unread_changed = matches!(action_label, "MarkRead");
    cx.spawn(async move |cx| {
        match rx.recv().await {
            Ok(Ok(())) => {
                if let Some(store) = store.upgrade() {
                    let _ = cx.update(|cx: &mut App| {
                        store.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                            // 标已读/未读：通知宿主刷新侧栏未读徽章
                            if unread_changed {
                                cx.emit(crate::stores::emails::EmailsEvent::UnreadChanged);
                            }
                        });
                    });
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(target: "postium::ui", "列表行操作失败（{action_label}）: {e}")
            }
            Err(_) => {}
        }
    })
    .detach();
}

/// 会话行级操作类型（Task 11）。
enum ConvAction {
    /// 会话星标切换（`toggle_conversation_star`，作用于最新一封）
    ToggleStar,
    /// 删除整组会话（会话内全部邮件走既有删除流）
    Delete,
}

/// 对指定会话执行异步操作，完成后刷新列表 store。
///
/// - **星标**：切换后重拉 `conversation_emails` 计算 `starred_any` 聚合
///   真值（其他封可能仍有星标，单看切换返回值不够），随后**原位补丁**
///   该行（对齐 T9 `mark_timeline_read` 路径：`list_version += 1` 原位
///   刷新，不动滚动/选中/会话展开态）。
/// - **删除**：`conversation_emails` 取会话内全部封，逐封走既有单封
///   删除流（async 串联——mail_operation::delete 第一阶段仅支持单封）→
///   `refresh_now` 重载会话档（INBOX 等文件夹过滤下封数归零的线程在
///   repo GROUP BY 自然消失）。被删会话若正是当前展开的时间线，经
///   MainShell 全局弱引用连带关闭详情面板。
fn act_on_conversation(
    api: Arc<CoreApi>,
    store: gpui_kit::WeakEntity<EmailsStore>,
    thread_id: i64,
    action: ConvAction,
    cx: &mut App,
) {
    let action_label = match action {
        ConvAction::ToggleStar => "ConvToggleStar",
        ConvAction::Delete => "ConvDelete",
    };
    let api_keep = api.clone();
    // 显式错误类型：两分支均以 Ok(bool) 收尾，`?` 的 From 约束不足以定 E
    let rx: smol::channel::Receiver<Result<bool, postium_core::error::MailError>> =
        spawn_tokio(async move {
            match action {
                ConvAction::ToggleStar => {
                    api.toggle_conversation_star(thread_id).await?;
                    let rows = api.conversation_emails(thread_id).await?;
                    Ok(rows.iter().any(|e| e.is_starred))
                }
                // 会话删除 = 对 conversation_emails 逐封走既有删除流
                // （async 串联——mail_operation::delete 第一阶段仅支持单封）。
                // 已在 Trash 等单封失败跳过续删（尽量收敛，不整体回滚）
                ConvAction::Delete => {
                    let rows = api.conversation_emails(thread_id).await?;
                    let mut deleted = false;
                    for email in rows {
                        match api.delete_emails(vec![email.id]).await {
                            Ok(_) => deleted = true,
                            Err(e) => tracing::warn!(
                                target: "postium::ui",
                                "会话删除单封跳过（thread {thread_id} email {}）: {e}",
                                email.id
                            ),
                        }
                    }
                    Ok(deleted)
                }
            }
        });
    cx.spawn(async move |cx| {
        match rx.recv().await {
            Ok(Ok(flag)) => {
                if let Some(store) = store.upgrade() {
                    let _ = cx.update(|cx: &mut App| match action_label {
                        "ConvToggleStar" => {
                            // 原位补丁 starred_any（聚合真值）
                            store.update(cx, |s, cx| {
                                if let Some(conv) = s
                                    .conversations
                                    .iter_mut()
                                    .find(|c| c.thread_id == thread_id)
                                {
                                    conv.starred_any = flag;
                                    s.list_version += 1;
                                    cx.notify();
                                }
                            });
                        }
                        _ => {
                            // 删除：会话档整体重载（线程自然消失；reset_and_load
                            // 已清会话选中）。若被删线程正展开时间线 → 关详情
                            let was_open = store.read(cx).selected_thread == Some(thread_id);
                            store.update(cx, |s, cx| {
                                s.refresh_now(&api_keep, cx);
                            });
                            if was_open {
                                close_thread_detail(cx);
                            }
                        }
                    });
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(target: "postium::ui", "会话行操作失败（{action_label}）: {e}")
            }
            Err(_) => {}
        }
    })
    .detach();
}

/// 关闭会话时间线详情面板（被删会话正展开时）：经 MainShell 全局弱引用
/// 清详情 store + 撤详情视图 + 恢复主窗口默认标题（对齐 app.rs 切档
/// 清理路径的清理集）。
fn close_thread_detail(cx: &mut App) {
    let Some(shell) = cx
        .try_global::<crate::app::MainShell>()
        .and_then(|g| g.0.upgrade())
    else {
        return;
    };
    shell.update(cx, |shell, cx| {
        shell.detail.update(cx, |s, cx| s.clear(cx));
        shell.detail_view = None;
        shell.update_window_title(cx);
    });
}

/// 构造会话行右键菜单：星标切换 ─ 删除会话（对齐平铺行菜单形态；
/// 回复/转发/标已读等单封语义操作不适用于会话级，不入菜单）。
fn build_conv_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    conv: Rc<ConversationSummary>,
    api: Arc<CoreApi>,
    store: gpui_kit::WeakEntity<EmailsStore>,
) -> gpui_kit::component::menu::PopupMenu {
    let thread_id = conv.thread_id;
    let starred_any = conv.starred_any;

    let star_item = {
        let (api, store) = (api.clone(), store.clone());
        PopupMenuItem::new(if starred_any {
            t("email.unstar").to_string()
        } else {
            t("email.star").to_string()
        })
        .icon(IconName::Star)
        .on_click(move |_, _, cx| {
            act_on_conversation(api.clone(), store.clone(), thread_id, ConvAction::ToggleStar, cx);
        })
    };
    let delete_item = {
        PopupMenuItem::new(t("email.deleteThread").to_string())
            .icon(IconName::Trash)
            .on_click(move |_, _, cx| {
                act_on_conversation(api.clone(), store.clone(), thread_id, ConvAction::Delete, cx);
            })
    };

    menu.item(star_item).separator().item(delete_item)
}

/// 回复种类：取详情后打开对应模式的写信窗口。
enum ReplyKind {
    Reply,
    ReplyAll,
    Forward,
}

/// 异步取邮件详情（正文引用/收件人），再打开写信窗口。
fn fetch_and_open_compose(api: Arc<CoreApi>, id: i32, kind: ReplyKind, cx: &mut App) {
    let api_keep = api.clone();
    let rx = spawn_tokio(async move { api.get_email(id).await });
    cx.spawn(async move |cx| {
        match rx.recv().await {
            Ok(Ok(detail)) => {
                // 回信线程头：References = 原 In-Reply-To 链 + 原 Message-ID（去重 `<id>` 逗号串）
                let message_id = detail.email.message_id.clone();
                let references = crate::views::compose::reply_references(
                    detail.email.in_reply_to.as_deref(),
                    detail.email.message_id.as_deref(),
                );
                let mode = match kind {
                    ReplyKind::Reply => crate::views::compose::ComposeMode::Reply {
                        email_id: detail.email.id,
                        account_id: detail.email.account_id,
                        to: detail.email.sender_email.clone(),
                        subject: detail.email.subject.clone().unwrap_or_default(),
                        original_html: detail.body_html.clone().unwrap_or_default(),
                        message_id,
                        references,
                    },
                    ReplyKind::ReplyAll => {
                        let mut to = vec![detail.email.sender_email.clone()];
                        for r in detail.recipient_emails.split([',', ';']) {
                            let r = r.trim();
                            if !r.is_empty() && !to.iter().any(|x| x.eq_ignore_ascii_case(r)) {
                                to.push(r.to_string());
                            }
                        }
                        crate::views::compose::ComposeMode::ReplyAll {
                            email_id: detail.email.id,
                            account_id: detail.email.account_id,
                            to: to.join(", "),
                            subject: detail.email.subject.clone().unwrap_or_default(),
                            original_html: detail.body_html.clone().unwrap_or_default(),
                            message_id,
                            references,
                        }
                    }
                    ReplyKind::Forward => crate::views::compose::ComposeMode::Forward {
                        email_id: detail.email.id,
                        account_id: detail.email.account_id,
                        subject: detail.email.subject.clone().unwrap_or_default(),
                        original_html: detail.body_html.clone().unwrap_or_default(),
                    },
                };
                let _ = cx.update(|cx: &mut App| {
                    crate::views::compose::open_compose_window(mode, &api_keep, cx);
                });
            }
            Ok(Err(e)) => {
                tracing::warn!(target: "postium::ui", "打开回复/转发窗口失败（取详情）: {e}")
            }
            Err(_) => {}
        }
    })
    .detach();
}

/// 构造邮件行右键菜单：回复/全部回复/转发 ─ 星标/已读 ─ 删除。
fn build_row_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    email: Rc<EmailDto>,
    api: Arc<CoreApi>,
    store: gpui_kit::WeakEntity<EmailsStore>,
) -> gpui_kit::component::menu::PopupMenu {
    let id = email.id;
    let starred = email.is_starred;
    let unread = !email.is_read;

    let reply_item = {
        let api = api.clone();
        PopupMenuItem::new(t("email.reply").to_string()).icon(IconName::Reply).on_click(move |_, _, cx| {
            fetch_and_open_compose(api.clone(), id, ReplyKind::Reply, cx);
        })
    };
    let reply_all_item = {
        let api = api.clone();
        PopupMenuItem::new(t("email.replyAll").to_string()).icon(IconName::Reply).on_click(move |_, _, cx| {
            fetch_and_open_compose(api.clone(), id, ReplyKind::ReplyAll, cx);
        })
    };
    let forward_item = {
        let api = api.clone();
        PopupMenuItem::new(t("email.forward").to_string()).icon(IconName::Forward).on_click(move |_, _, cx| {
            fetch_and_open_compose(api.clone(), id, ReplyKind::Forward, cx);
        })
    };
    let star_item = {
        let (api, store) = (api.clone(), store.clone());
        PopupMenuItem::new(if starred { "取消星标".to_string() } else { "设为星标".to_string() })
            .icon(IconName::Star)
            .on_click(move |_, _, cx| {
                act_on_email(api.clone(), store.clone(), id, RowAction::ToggleStar, cx);
            })
    };
    let read_item = {
        let (api, store) = (api.clone(), store.clone());
        PopupMenuItem::new(if unread { "标为已读".to_string() } else { "标为未读".to_string() })
            .icon(IconName::MailOpen)
            .on_click(move |_, _, cx| {
                // unread 即目标已读状态：未读(true)→标已读(true)、已读(false)→标未读(false)
                act_on_email(api.clone(), store.clone(), id, RowAction::MarkRead(unread), cx);
            })
    };
    let delete_item = {
        PopupMenuItem::new(t("email.delete").to_string()).icon(IconName::Trash).on_click(move |_, _, cx| {
            act_on_email(api.clone(), store.clone(), id, RowAction::Delete, cx);
        })
    };

    menu
        .item(reply_item)
        .item(reply_all_item)
        .item(forward_item)
        .separator()
        .item(star_item)
        .item(read_item)
        .separator()
        .item(delete_item)
}

/// 委托侧行数据（统一载体）：平铺档单封邮件 / 会话档线程聚合。
/// 两种形态不混排（由 [`EmailsStore::in_conversation_view`] 决定），
/// 宿主整体替换行集合（`set_rows`/`set_conversations`）。
enum ListRow {
    Email(Rc<EmailDto>),
    Conversation(Rc<ConversationSummary>),
}

pub struct EmailListDelegate {
    rows: Vec<ListRow>,
    selected_email: Option<i32>,
    /// 会话档选中线程 id（与 selected_email 互斥；行集合整体换档时
    /// 由 set_rows/set_conversations 清对侧选中）
    selected_thread: Option<i64>,
    store: gpui_kit::WeakEntity<EmailsStore>,
    api: Arc<postium_core::api::CoreApi>,
    /// 「加载更早邮件」服务器同步中（防重复触发；footer 按钮显示态）
    older_syncing: bool,
}

impl EmailListDelegate {
    /// 原位替换平铺行数据（翻页/刷新路径，保持滚动与选中）。
    /// 选中 id 不在新行集合中时清除选中；行集合换回平铺档时清会话选中。
    pub fn set_rows(&mut self, rows: Vec<Rc<EmailDto>>) {
        if let Some(sel) = self.selected_email
            && !rows.iter().any(|r| r.id == sel)
        {
            self.selected_email = None;
        }
        self.selected_thread = None;
        self.rows = rows.into_iter().map(ListRow::Email).collect();
    }

    /// 原位替换会话行数据（会话档翻页/刷新路径，语义同 [`Self::set_rows`]）。
    pub fn set_conversations(&mut self, rows: Vec<Rc<ConversationSummary>>) {
        if let Some(sel) = self.selected_thread
            && !rows.iter().any(|r| r.thread_id == sel)
        {
            self.selected_thread = None;
        }
        self.selected_email = None;
        self.rows = rows.into_iter().map(ListRow::Conversation).collect();
    }
}

impl ListDelegate for EmailListDelegate {
    type Item = EmailListRow;

    fn sections_count(&self, _: &App) -> usize {
        1
    }

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.rows.len()
    }

    fn perform_search(
        &mut self,
        _query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        // 搜索由顶栏搜索框 → EmailsStore::search 驱动，列表内禁用
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        match ix.and_then(|i| self.rows.get(i.row)) {
            Some(ListRow::Email(email)) => {
                self.selected_email = Some(email.id);

                // 选中即打开：写回 store，宿主据此渲染详情
                if let Some(store) = self.store.upgrade() {
                    let selected = self.selected_email;
                    store.update(cx, |store, cx| {
                        let changed = store.selected != selected;
                        store.selected = selected;
                        store.selected_version += 1;
                        cx.notify();
                        if changed
                            && let Some(id) = selected
                        {
                            cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(id));
                        }
                    });

                    // 异步标记已读：完成后原位更新行内 is_read 标记 + 通知侧栏刷新徽章
                    let (id, was_unread) = (email.id, !email.is_read);
                    if was_unread {
                        let api = self.api.clone();
                        let rx = spawn_tokio(async move { api.mark_as_read(id, true).await });
                        let row_ix = ix.map(|i| i.row);
                        cx.spawn(async move |this, cx| {
                            if rx.recv().await.is_ok() {
                                this.update(cx, |state, cx| {
                                    let delegate = state.delegate_mut();
                                    if let Some(row) = row_ix
                                        .and_then(|ri| delegate.rows.get_mut(ri))
                                        && let ListRow::Email(row) = row
                                        && row.id == id
                                    {
                                        let mut updated = (**row).clone();
                                        updated.is_read = true;
                                        *row = std::rc::Rc::new(updated);
                                    }
                                    cx.notify();
                                })
                                .ok();
                                // 通知宿主刷新文件夹统计（侧栏未读徽章）
                                store.update(cx, |_, cx| {
                                    cx.emit(crate::stores::emails::EmailsEvent::UnreadChanged);
                                });
                            }
                        })
                        .detach();
                    }
                }
            }
            Some(ListRow::Conversation(conv)) => {
                self.selected_thread = Some(conv.thread_id);

                // 会话选中即打开：写回 store（清单封选中 + 发
                // ThreadSelectionChanged），会话时间线由 T10 宿主接线装载
                if let Some(store) = self.store.upgrade() {
                    let thread_id = conv.thread_id;
                    store.update(cx, |store, cx| {
                        store.select_thread(thread_id, cx);
                    });
                }
            }
            None => {
                self.selected_email = None;
                self.selected_thread = None;
                if let Some(store) = self.store.upgrade() {
                    store.update(cx, |store, cx| {
                        // 与既有平铺清选语义一致：只推进版本，不发事件
                        store.selected = None;
                        store.selected_version += 1;
                        if store.selected_thread.take().is_some() {
                            store.selected_thread_version += 1;
                        }
                        cx.notify();
                    });
                }
            }
        }
        cx.notify();
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        match self.rows.get(ix.row)? {
            ListRow::Email(email) => Some(EmailListRow::Email(EmailRow {
                base: ListItem::new(ix).selected(Some(email.id) == self.selected_email),
                email: email.clone(),
                selected: Some(email.id) == self.selected_email,
                api: Some(self.api.clone()),
                store: Some(self.store.clone()),
            })),
            ListRow::Conversation(conv) => Some(EmailListRow::Conversation(ConversationRow {
                base: ListItem::new(ix).selected(Some(conv.thread_id) == self.selected_thread),
                conversation: conv.clone(),
                selected: Some(conv.thread_id) == self.selected_thread,
                api: Some(self.api.clone()),
                store: Some(self.store.clone()),
            })),
        }
    }

    /// 列表末尾的加载按钮（合并「加载更多」/「加载更早邮件」）。
    ///
    /// 作为 section footer 物理位于列表末尾——滚到底才可见，列表
    /// 不满一屏（邮件少）时自然显示，无需滚动监听。
    fn render_section_footer(
        &mut self,
        _: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let store_entity = self.store.upgrade()?;
        let can_more = {
            let store = store_entity.read(cx);
            // 分页闸门按当前行集合档位（会话 or 邮件）判定
            if store.in_conversation_view() {
                (store.conversations.len() as u64) < store.conversation_total
            } else {
                (store.emails.len() as u64) < store.total
            }
        };
        let label = if self.older_syncing {
            t("email.syncingOlder")
        } else if can_more {
            t("email.loadMore")
        } else {
            t("email.syncOlder")
        };

        let store = self.store.clone();
        let api = self.api.clone();
        let state = cx.weak_entity();
        let mut btn = Button::new("list-load-footer").ghost().xsmall();
        if self.older_syncing {
            btn = btn.disabled(true);
        } else {
            btn = btn.on_click(move |_, _, cx| {
                if can_more {
                    let _ = store.update(cx, |s, cx| s.load_more(&api, cx));
                } else if let Some(state) = state.upgrade() {
                    let _ = state.update(cx, |state, cx| state.delegate_mut().start_sync_older(cx));
                }
            });
        }
        btn = btn.label(label.to_string());

        Some(
            h_flex()
                .w_full()
                .justify_center()
                .py_2()
                .child(btn),
        )
    }
}

impl EmailListDelegate {
    /// 从服务器同步更早邮件（本地数据到底后 footer 按钮触发）。
    fn start_sync_older(&mut self, cx: &mut Context<ListState<Self>>) {
        if self.older_syncing {
            return;
        }
        self.older_syncing = true;
        cx.notify();
        let nav = self.store.upgrade().map(|s| s.read(cx).nav.clone());
        let api = self.api.clone();
        let api_keep = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move {
            match &nav {
                Some(NavTarget::AccountFolder { account_id, .. }) => {
                    api.sync_older_emails(*account_id, EmailCategory::Inbox).await.map(|_| ())
                }
                Some(NavTarget::Category(c)) => {
                    let accounts = api.list_accounts().await.unwrap_or_default();
                    for a in accounts {
                        let _ = api.sync_older_emails(a.id, c.clone()).await;
                    }
                    Ok(())
                }
                // 全部收件箱：对全部账号同步收件箱更早邮件
                Some(NavTarget::AllInbox) => {
                    let accounts = api.list_accounts().await.unwrap_or_default();
                    for a in accounts {
                        let _ = api.sync_older_emails(a.id, EmailCategory::Inbox).await;
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        });
        cx.spawn(async move |this, cx| {
            if rx.recv().await.is_ok() && let Some(store) = store.upgrade() {
                store.update(cx, |s, cx| {
                    s.refresh_now(&api_keep, cx);
                });
            }
            if let Some(this) = this.upgrade() {
                this.update(cx, |state, cx| {
                    state.delegate_mut().older_syncing = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

pub struct EmailListView {
    pub state: Entity<ListState<EmailListDelegate>>,
    pub store: Entity<EmailsStore>,
    pub api: Arc<CoreApi>,
}

enum SelectedAction {
    Star,
    ToggleRead(bool),
    Archive,
    Delete,
    Reload,
}

impl EmailListView {
    /// 对选中邮件执行操作并刷新列表。
    fn act_on_selected(&mut self, action: SelectedAction, cx: &mut Context<Self>) {
        let Some(id) = self.store.read(cx).selected else { return };
        let api = self.api.clone();
        let api_keep = self.api.clone();
        let store = self.store.clone();
        let unread_changed = matches!(action, SelectedAction::ToggleRead(_));
        let rx = spawn_tokio(async move {
            match action {
                SelectedAction::Star => api.toggle_star(id).await.map(|_| ()),
                SelectedAction::ToggleRead(read) => api.mark_as_read(id, read).await,
                SelectedAction::Archive => api.archive_email(id).await,
                SelectedAction::Delete => api.delete_emails(vec![id]).await.map(|_| ()),
                SelectedAction::Reload => api.reload_email(id).await.map(|_| ()),
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                let _ = result;
                store.update(cx, |store, cx| {
                    store.refresh_now(&api_keep, cx);
                    // 标已读/未读：通知宿主刷新侧栏未读徽章
                    if unread_changed {
                        cx.emit(crate::stores::emails::EmailsEvent::UnreadChanged);
                    }
                });
                this.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();
    }

    fn toggle_unread_filter(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| {
            store.unread_only = !store.unread_only;
            store.refresh_now(&api, cx);
        });
        cx.notify();
    }

    /// 会话/平铺视图档切换：持久化偏好并按新档位重装列表（退出搜索
    /// 上下文）。宿主据 nav_version 重建整个列表视图。
    fn toggle_view_mode(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| {
            let next = if store.view_mode == EmailViewMode::Threaded {
                EmailViewMode::Flat
            } else {
                EmailViewMode::Threaded
            };
            store.set_view_mode(next, &api, cx);
        });
        cx.notify();
    }

    fn forward_selected(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.store.read(cx).selected else { return };
        let api = self.api.clone();
        let api2 = self.api.clone();
        let rx = spawn_tokio(async move { api.get_email(id).await });
        cx.spawn(async move |_, cx| {
            match rx.recv().await {
                Ok(Ok(detail)) => {
                    let subject = detail.email.subject.clone().unwrap_or_default();
                    let original_html = detail.body_html.clone().unwrap_or_default();
                    let email_id = detail.email.id;
                    let account_id = detail.email.account_id;
                    cx.update(|cx| {
                        crate::views::compose::open_compose_window(
                            crate::views::compose::ComposeMode::Forward {
                                email_id,
                                account_id,
                                subject,
                                original_html,
                            },
                            &api2,
                            cx,
                        );
                    });
                }
                Ok(Err(e)) => {
                    tracing::warn!(target: "postium::ui", "打开转发窗口失败（取详情）: {e}")
                }
                Err(_) => {}
            }
        })
        .detach();
    }
}

impl Render for EmailListView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        let has_selected = store.selected.is_some();
        let selected_unread = store
            .emails
            .iter()
            .find(|e| Some(e.id) == store.selected)
            .map(|e| !e.is_read)
            .unwrap_or(false);
        // 会话/平铺切换：图标与提示指向点击后的目标档位（与未读过滤
        // 按钮的目标态语义一致）；搜索态仍可切档（切档退出搜索重装导航视图）
        let threaded_now = store.view_mode == EmailViewMode::Threaded;
        let (mode_icon, mode_tip) = if threaded_now {
            (IconName::List, t("email.flatView"))
        } else {
            (IconName::MessageSquare, t("email.threadedView"))
        };
        let conversation_ctx = store.in_conversation_view();
        let mut toolbar = h_flex()
            .gap_1()
            .items_center()
            .flex_none()
            .child(
                Button::new("unread-filter")
                    .ghost()
                    .xsmall()
                    .label(if store.unread_only { "全部" } else { "未读" })
                    .tooltip("切换未读/全部")
                    .on_click(cx.listener(Self::toggle_unread_filter)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    // 计数按当前行集合档位（会话 or 邮件）
                    .child(if conversation_ctx {
                        format!(
                            "{} / {}",
                            store.conversations.len(),
                            store.conversation_total
                        )
                    } else {
                        format!("{} / {}", store.emails.len(), store.total)
                    }),
            );

        if has_selected {
            toolbar = toolbar
                .child(
                    Button::new("sel-star")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Star)
                        .tooltip("星标切换")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Star, cx);
                        })),
                )
                .child(
                    Button::new("sel-read")
                        .ghost()
                        .xsmall()
                        .label(if selected_unread { "标已读" } else { "标未读" })
                        .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                            // selected_unread 即目标已读状态（切换语义）
                            this.act_on_selected(SelectedAction::ToggleRead(selected_unread), cx);
                        })),
                )
                .child(
                    Button::new("sel-reload")
                        .ghost()
                        .xsmall()
                        .icon(IconName::RotateCw)
                        .tooltip("重新加载")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Reload, cx);
                        })),
                )
                .child(
                    Button::new("sel-forward")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Forward)
                        .tooltip("转发")
                        .on_click(cx.listener(Self::forward_selected)),
                )
                .child(
                    Button::new("sel-archive")
                        .ghost()
                        .xsmall()
                        .label("归档")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Archive, cx);
                        })),
                )
                .child(
                    Button::new("sel-delete")
                        .ghost()
                        .xsmall()
                        .label("删除")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Delete, cx);
                        })),
                );
        }

        // 会话/平铺切档按钮固定在工具条右端
        toolbar = toolbar
            .child(div().flex_1())
            .child(
                Button::new("view-mode-toggle")
                    .ghost()
                    .xsmall()
                    .icon(mode_icon)
                    .tooltip(mode_tip)
                    .on_click(cx.listener(Self::toggle_view_mode))
                    .debug_selector(|| "view-mode-toggle".to_string()),
            );

        // 加载按钮在列表末尾（delegate render_section_footer）：滚到底才可见
        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(toolbar)
            .child(List::new(&self.state).flex_1())
    }
}

/// 用 EmailsStore 数据重建列表状态（行数据源按 [`EmailsStore::in_conversation_view`]
/// 分派：会话档取 `conversations`，否则平铺 `emails`）。
pub fn rebuild_list_state(
    store: &Entity<EmailsStore>,
    api: &Arc<postium_core::api::CoreApi>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ListState<EmailListDelegate>> {
    let (rows, selected_email, selected_thread) = {
        let s = store.read(cx);
        if s.in_conversation_view() {
            (
                s.conversations
                    .iter()
                    .cloned()
                    .map(|c| ListRow::Conversation(Rc::new(c)))
                    .collect::<Vec<_>>(),
                None,
                s.selected_thread,
            )
        } else {
            (
                s.emails
                    .iter()
                    .cloned()
                    .map(|e| ListRow::Email(Rc::new(e)))
                    .collect::<Vec<_>>(),
                s.selected,
                None,
            )
        }
    };
    let weak = store.downgrade();
    cx.new(|cx| {
        ListState::new(
            EmailListDelegate {
                rows,
                selected_email,
                selected_thread,
                store: weak,
                api: api.clone(),
                older_syncing: false,
            },
            window,
            cx,
        )
    })
}

/// 供 e2e 驱动右键菜单动作的测试钩子（不经选中态，直连行级动作链路）。
#[cfg(test)]
pub(crate) mod test_hooks {
    use super::*;

    pub(crate) enum RowActionForTest {
        ToggleStar,
        MarkRead(bool),
        Delete,
    }

    pub(crate) fn act_on_email_for_test(
        api: Arc<CoreApi>,
        id: i32,
        action: RowActionForTest,
        cx: &mut App,
    ) {
        let mapped = match action {
            RowActionForTest::ToggleStar => RowAction::ToggleStar,
            RowActionForTest::MarkRead(read) => RowAction::MarkRead(read),
            RowActionForTest::Delete => RowAction::Delete,
        };
        // 悬空 store：DB 动作仍执行，仅跳过列表刷新
        act_on_email(api, gpui_kit::WeakEntity::new_invalid(), id, mapped, cx);
    }

    pub(crate) enum ConvActionForTest {
        ToggleStar,
        Delete,
    }

    /// 会话行动作测试钩子：直连 [`act_on_conversation`] 动作链路（真实
    /// store 弱引用，星标原位补丁 / 删除 refresh_now 均可观测）。
    pub(crate) fn act_on_conversation_for_test(
        api: Arc<CoreApi>,
        store: gpui_kit::WeakEntity<EmailsStore>,
        thread_id: i64,
        action: ConvActionForTest,
        cx: &mut App,
    ) {
        let mapped = match action {
            ConvActionForTest::ToggleStar => ConvAction::ToggleStar,
            ConvActionForTest::Delete => ConvAction::Delete,
        };
        act_on_conversation(api, store, thread_id, mapped, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::participants_summary_text;

    fn summary(list: &[&str]) -> Option<String> {
        let owned: Vec<String> = list.iter().map(|s| (*s).to_string()).collect();
        participants_summary_text(&owned)
    }

    #[test]
    fn participants_summary_renders_up_to_three_then_plus_n() {
        assert_eq!(summary(&["c@x"]), Some("c@x".to_string()));
        assert_eq!(summary(&["c@x", "a@x"]), Some("c@x, a@x".to_string()));
        assert_eq!(
            summary(&["c@x", "a@x", "b@x"]),
            Some("c@x, a@x, b@x".to_string()),
            "恰 3 个：全展示，无计数段"
        );
        assert_eq!(
            summary(&["c@x", "a@x", "b@x", "d@x"]),
            Some("c@x, a@x, b@x +1".to_string()),
            "第 4 个起折叠为 +N"
        );
        assert_eq!(
            summary(&["c@x", "a@x", "b@x", "d@x", "e@x"]),
            Some("c@x, a@x, b@x +2".to_string())
        );
    }

    #[test]
    fn participants_summary_empty_returns_none_for_sender_fallback() {
        assert_eq!(summary(&[]), None, "空参与者摘要省略段（回退发件人展示）");
    }
}

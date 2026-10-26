//! 日历视图（移植自 src/routes/(main)/calendar/+page.svelte 的精简版）：
//! 按日期分组的邮件时间线。

use std::sync::Arc;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use postium_core::api::CoreApi;
use postium_core::service::email_service::{EmailCategory, EmailDto};

use crate::i18n::t;
use crate::runtime::spawn_tokio;

pub struct CalendarView {
    pub api: Arc<CoreApi>,
    emails: Vec<EmailDto>,
    loaded: bool,
}

impl CalendarView {
    pub fn new(api: Arc<CoreApi>, cx: &mut Context<Self>) -> Self {
        let rx = spawn_tokio({
            let api = api.clone();
            async move {
                api.list_emails_by_category_for_all_accounts(EmailCategory::Inbox, 0, 200, false)
                    .await
                    .map(|r| r.emails)
                    .unwrap_or_default()
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(emails) = rx.recv().await {
                this.update(cx, |view, cx| {
                    view.emails = emails;
                    view.loaded = true;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        Self { api, emails: Vec::new(), loaded: false }
    }
}

impl Render for CalendarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 按日期分组
        let mut groups: Vec<(String, Vec<&EmailDto>)> = Vec::new();
        for email in &self.emails {
            let date = chrono::DateTime::from_timestamp(email.sent_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %A").to_string())
                .unwrap_or_else(|| "未知日期".into());
            match groups.last_mut() {
                Some((d, list)) if *d == date => list.push(email),
                _ => groups.push((date, vec![email])),
            }
        }

        let mut list = v_flex()
            .size_full()
            .p_4()
            .gap_2()
            .id("calendar-scroll")
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::BOLD)
                    .child(t("sidebar.calendar")),
            );

        if !self.loaded {
            list = list.child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("email.loading")),
            );
        } else if groups.is_empty() {
            list = list.child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("email.noEmails")),
            );
        }

        for (date, emails) in groups {
            list = list.child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .pt_2()
                    .child(date),
            );
            for e in emails {
                list = list.child(
                    h_flex()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(cx.theme().muted_foreground)
                                .w(px(44.))
                                .child(
                                    chrono::DateTime::from_timestamp(e.sent_at, 0)
                                        .map(|dt| dt.format("%H:%M").to_string())
                                        .unwrap_or_default(),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .flex_1()
                                .line_clamp(1)
                                .child(e.subject.clone().unwrap_or_else(|| "（无主题）".into())),
                        ),
                );
            }
        }
        list
    }
}

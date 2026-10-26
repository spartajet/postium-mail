//! 工作流视图（移植自 workflow/+page.svelte 的精简版）：
//! 未读/星标事项的跟进列表。

use std::sync::Arc;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::{EmailCategory, EmailDto};

use crate::i18n::t;
use crate::runtime::spawn_tokio;

pub struct WorkflowView {
    pub api: Arc<CoreApi>,
    starred: Vec<EmailDto>,
    loaded: bool,
}

impl WorkflowView {
    pub fn new(api: Arc<CoreApi>, cx: &mut Context<Self>) -> Self {
        let rx = spawn_tokio({
            let api = api.clone();
            async move {
                api.list_emails_by_category_for_all_accounts(EmailCategory::Starred, 0, 100, false)
                    .await
                    .map(|r| r.emails)
                    .unwrap_or_default()
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(emails) = rx.recv().await {
                this.update(cx, |view, cx| {
                    view.starred = emails;
                    view.loaded = true;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        Self { api, starred: Vec::new(), loaded: false }
    }
}

impl Render for WorkflowView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex()
            .size_full()
            .p_4()
            .gap_2()
            .id("workflow-scroll")
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::BOLD)
                    .child(t("sidebar.workflow")),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child("星标待跟进："),
            );

        if !self.loaded {
            list = list.child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("email.loading")),
            );
        } else if self.starred.is_empty() {
            list = list.child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("email.noEmails")),
            );
        }

        for e in &self.starred {
            list = list.child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        gpui_kit::component::Icon::new(IconName::Star)
                            .size_3()
                            .text_color(gpui_kit::rgb(0xf59e0b)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .flex_1()
                            .line_clamp(1)
                            .child(e.subject.clone().unwrap_or_else(|| "（无主题）".into())),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(e.sender_name.clone().unwrap_or_else(|| e.sender_email.clone())),
                    ),
            );
        }
        list
    }
}

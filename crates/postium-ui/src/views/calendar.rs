//! 日历视图（对齐旧版 calendar/+page.svelte：月历网格 + 星期头 + 选中日邮件列表）。

use std::sync::Arc;

use chrono::{Datelike, Duration, Local};
use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::{EmailCategory, EmailDto};

use crate::i18n::t;
use crate::runtime::spawn_tokio;

pub struct CalendarView {
    pub api: Arc<CoreApi>,
    emails: Vec<EmailDto>,
    loaded: bool,
    /// 当前展示的年月
    year: i32,
    month: u32, // 1-12
    selected_day: Option<u32>,
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
        let today = Local::now().date_naive();
        Self {
            api,
            emails: Vec::new(),
            loaded: false,
            year: today.year(),
            month: today.month(),
            selected_day: None,
        }
    }

    fn prev_month(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (y, m) = if self.month == 1 { (self.year - 1, 12) } else { (self.year, self.month - 1) };
        self.year = y;
        self.month = m;
        self.selected_day = None;
        cx.notify();
    }

    fn next_month(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (y, m) = if self.month == 12 { (self.year + 1, 1) } else { (self.year, self.month + 1) };
        self.year = y;
        self.month = m;
        self.selected_day = None;
        cx.notify();
    }

    fn pick_day(&mut self, day: u32, cx: &mut Context<Self>) {
        self.selected_day = Some(day);
        cx.notify();
    }

    fn email_day(&self, e: &EmailDto) -> Option<chrono::NaiveDate> {
        chrono::DateTime::from_timestamp(e.sent_at, 0).map(|dt| dt.date_naive())
    }

    fn day_has_email(&self, d: u32) -> bool {
        self.emails.iter().any(|e| {
            self.email_day(e)
                .map(|date| date.year() == self.year && date.month() == self.month && date.day() == d)
                .unwrap_or(false)
        })
    }
}

impl Render for CalendarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 月历参数
        let days_in_month = {
            let (y, m) = (self.year, self.month);
            let first_next = chrono::NaiveDate::from_ymd_opt(
                if m == 12 { y + 1 } else { y },
                if m == 12 { 1 } else { m + 1 },
                1,
            )
            .unwrap();
            (first_next - Duration::days(1)).day()
        };
        let first_weekday = chrono::NaiveDate::from_ymd_opt(self.year, self.month, 1)
            .unwrap()
            .weekday()
            .num_days_from_sunday(); // 周日=0

        // 星期头
        let mut days_grid = v_flex().gap_1();
        let week_labels = ["日", "一", "二", "三", "四", "五", "六"];
        let mut header = h_flex().gap_1();
        for w in week_labels {
            header = header.child(
                div()
                    .w(px(36.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(w),
            );
        }
        days_grid = days_grid.child(header);

        // 日期格
        let mut week = h_flex().gap_1();
        let mut cell_index = 0usize;
        for day in 1..=days_in_month {
            let slot = if day == 1 { first_weekday } else { 0 };
            for _ in 0..slot {
                week = week.child(div().w(px(36.)).h(px(36.)));
                cell_index += 1;
            }
            let has_email = self.day_has_email(day);
            let selected = self.selected_day == Some(day);
            let today = {
                let now = Local::now().date_naive();
                now.year() == self.year && now.month() == self.month && now.day() == day
            };
            week = week.child(
                div()
                    .id(("day", day))
                    .w(px(36.))
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .rounded(cx.theme().radius)
                    .when(selected, |this| this.bg(cx.theme().accent))
                    .when(!selected && today, |this| {
                        this.border_1().border_color(cx.theme().accent)
                    })
                    .when(has_email && !selected, |this| {
                        this.bg(cx.theme().accent.opacity(0.25))
                    })
                    .child(day.to_string())
                    .on_click({
                        let day = day;
                        cx.listener(move |view: &mut Self, _, _, cx| {
                            view.pick_day(day, cx);
                        })
                    }),
            );
            cell_index += 1;
            if cell_index % 7 == 0 {
                days_grid = days_grid.child(week);
                week = h_flex().gap_1();
            }
        }
        if cell_index % 7 != 0 {
            days_grid = days_grid.child(week);
        }

        // 选中日邮件
        let selected_emails: Vec<&EmailDto> = self
            .emails
            .iter()
            .filter(|e| {
                self.selected_day
                    .map(|d| {
                        self.email_day(e)
                            .map(|date| date.day() == d && date.month() == self.month && date.year() == self.year)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            })
            .collect();

        let mut timeline = v_flex().gap_1().flex_1().min_w(px(280.));
        if self.selected_day.is_some() {
            for e in &selected_emails {
                timeline = timeline.child(
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
            if selected_emails.is_empty() {
                timeline = timeline.child(
                    div()
                        .text_size(px(12.5))
                        .text_color(cx.theme().muted_foreground)
                        .child(t("email.noEmails")),
                );
            } else {
                let _ = &selected_emails;
            }
        } else {
            timeline = timeline.child(
                div()
                    .text_size(px(12.5))
                    .text_color(cx.theme().muted_foreground)
                    .child("选择日期查看当天邮件"),
            );
        }

        h_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                v_flex()
                    .flex_none()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("prev-month")
                                    .ghost()
                                    .icon(IconName::ChevronLeft)
                                    .on_click(cx.listener(Self::prev_month)),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("{}-{:02}", self.year, self.month)),
                            )
                            .child(
                                Button::new("next-month")
                                    .ghost()
                                    .icon(IconName::ChevronRight)
                                    .on_click(cx.listener(Self::next_month)),
                            ),
                    )
                    .child(days_grid),
            )
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .id("calendar-list")
                    .overflow_y_scroll()
                    .child(timeline),
            )
    }
}

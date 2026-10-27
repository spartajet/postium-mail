//! 日历视图（邮件时间轴）：月历作为邮件的日期索引。
//! 设计：大格子（52px）+ 当日邮件数徽标 + 未读强调 + 周末淡色 +
//! 头部（导航/今天/本月统计）+ 右侧当日邮件富卡片。

use std::sync::Arc;

use chrono::{Datelike, Duration, Local, NaiveDate};
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

const CELL: f32 = 52.0;
const WEEK_LABELS: [&str; 7] = ["日", "一", "二", "三", "四", "五", "六"];

pub struct CalendarView {
    #[allow(dead_code)]
    pub api: Arc<CoreApi>,
    emails: Vec<EmailDto>,
    loaded: bool,
    year: i32,
    month: u32, // 1-12
    selected_day: Option<u32>,
}

/// 某日的邮件统计。
struct DayStat {
    count: usize,
    unread: usize,
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

    fn go_today(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let today = Local::now().date_naive();
        self.year = today.year();
        self.month = today.month();
        self.selected_day = Some(today.day());
        cx.notify();
    }

    fn pick_day(&mut self, day: u32, cx: &mut Context<Self>) {
        self.selected_day = Some(day);
        cx.notify();
    }

    fn email_date(&self, e: &EmailDto) -> Option<NaiveDate> {
        chrono::DateTime::from_timestamp(e.sent_at, 0).map(|dt| dt.date_naive())
    }

    /// 指定日期的邮件统计。
    fn stat_of(&self, day: u32) -> DayStat {
        let mut count = 0;
        let mut unread = 0;
        for e in &self.emails {
            if let Some(d) = self.email_date(e)
                && d.year() == self.year
                && d.month() == self.month
                && d.day() == day
            {
                count += 1;
                if !e.is_read {
                    unread += 1;
                }
            }
        }
        DayStat { count, unread }
    }

    /// 本月统计。
    fn month_stat(&self) -> (usize, usize) {
        let mut count = 0;
        let mut unread = 0;
        for e in &self.emails {
            if let Some(d) = self.email_date(e)
                && d.year() == self.year
                && d.month() == self.month
            {
                count += 1;
                if !e.is_read {
                    unread += 1;
                }
            }
        }
        (count, unread)
    }

    /// 选中日的邮件（时间升序）。
    fn selected_emails(&self) -> Vec<&EmailDto> {
        let Some(day) = self.selected_day else { return Vec::new() };
        let mut list: Vec<&EmailDto> = self
            .emails
            .iter()
            .filter(|e| {
                self.email_date(e)
                    .map(|d| d.day() == day && d.month() == self.month && d.year() == self.year)
                    .unwrap_or(false)
            })
            .collect();
        list.sort_by_key(|e| e.sent_at);
        list
    }
}

impl Render for CalendarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let today = Local::now().date_naive();

        // ── 月历参数 ──
        let days_in_month = {
            let next_y = if self.month == 12 { self.year + 1 } else { self.year };
            let next_m = if self.month == 12 { 1 } else { self.month + 1 };
            let first_next = NaiveDate::from_ymd_opt(next_y, next_m, 1).unwrap();
            (first_next - Duration::days(1)).day()
        };
        let first_weekday =
            NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap().weekday().num_days_from_sunday();

        // ── 头部：标题 + 月份导航 + 今天 + 本月统计 ──
        let (m_count, m_unread) = self.month_stat();
        let month_title = format!("{}年{}月", self.year, self.month);
        let header = h_flex()
            .flex_none()
            .items_center()
            .gap_2()
            .pb_3()
            .child(
                gpui_kit::component::Icon::new(IconName::Calendar)
                    .size_5()
                    .text_color(theme.accent),
            )
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::BOLD)
                    .min_w(px(110.))
                    .child(month_title),
            )
            .child(
                Button::new("prev-month")
                    .ghost()
                    .icon(IconName::ChevronLeft)
                    .on_click(cx.listener(Self::prev_month)),
            )
            .child(
                Button::new("next-month")
                    .ghost()
                    .icon(IconName::ChevronRight)
                    .on_click(cx.listener(Self::next_month)),
            )
            .child(
                Button::new("go-today")
                    .ghost()
                    .label("今天")
                    .on_click(cx.listener(Self::go_today)),
            )
            .child(
                div()
                    .ml_auto()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(format!("本月 {} 封 · 未读 {}", m_count, m_unread)),
            );

        // ── 星期头（周末列 accent 淡色）──
        let mut grid = v_flex().gap(px(3.));
        let mut header_row = h_flex().gap(px(3.));
        for (i, w) in WEEK_LABELS.iter().enumerate() {
            header_row = header_row.child(
                div()
                    .w(px(CELL))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(if i == 0 || i == 6 {
                        theme.accent.opacity(0.7)
                    } else {
                        theme.muted_foreground
                    })
                    .child(*w),
            );
        }
        grid = grid.child(header_row);

        // ── 日期格 ──
        let mut week = h_flex().gap(px(3.));
        let mut cell_index = 0usize;
        for day in 1..=days_in_month {
            if day == 1 {
                for _ in 0..first_weekday {
                    week = week.child(div().w(px(CELL)).h(px(CELL)).rounded(theme.radius));
                    cell_index += 1;
                }
            }
            let stat = self.stat_of(day);
            let is_today = today.year() == self.year && today.month() == self.month && today.day() == day;
            let selected = self.selected_day == Some(day);
            let is_weekend = cell_index % 7 == 0 || cell_index % 7 == 6;

            let num_color = if selected {
                theme.accent_foreground
            } else if stat.unread > 0 {
                theme.accent
            } else if is_weekend {
                theme.muted_foreground
            } else {
                theme.foreground
            };
            let num_weight = if stat.unread > 0 || is_today { FontWeight::BOLD } else { FontWeight::NORMAL };

            let badge_bg = if selected {
                gpui_kit::white().opacity(0.3)
            } else {
                theme.accent.opacity(0.85)
            };

            let cell = div()
                .id(("day", day))
                .relative()
                .w(px(CELL))
                .h(px(CELL))
                .p(px(4.))
                .rounded(theme.radius)
                .when(selected, |d| d.bg(theme.accent))
                .when(is_today && !selected, |d| d.border_1().border_color(theme.accent))
                .when(!selected && !is_today, |d| d.hover(|s| s.bg(theme.accent.opacity(0.12))))
                .cursor_pointer()
                .on_click({
                    let day = day;
                    cx.listener(move |view: &mut Self, _, _, cx| {
                        view.pick_day(day, cx);
                    })
                })
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(num_weight)
                        .text_color(num_color)
                        .child(day.to_string()),
                )
                // 当日邮件数徽标（右下）
                .when(stat.count > 0, |d| {
                    d.child(
                        div()
                            .absolute()
                            .bottom(px(4.))
                            .right(px(4.))
                            .px(px(4.))
                            .rounded(px(7.))
                            .bg(badge_bg)
                            .text_color(theme.accent_foreground)
                            .text_size(px(9.5))
                            .child(stat.count.to_string()),
                    )
                });

            week = week.child(cell);
            cell_index += 1;
            if cell_index % 7 == 0 {
                grid = grid.child(week);
                week = h_flex().gap(px(3.));
            }
        }
        if cell_index % 7 != 0 {
            grid = grid.child(week);
        }

        let month_panel = v_flex()
            .flex_none()
            .p_4()
            .border_r_1()
            .border_color(theme.border)
            .child(header)
            .child(grid);

        // ── 右侧：当日面板 ──
        let weekday_names = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
        let (title, state) = match self.selected_day {
            Some(day) => {
                let date = NaiveDate::from_ymd_opt(self.year, self.month, day).unwrap();
                let weekday = weekday_names[date.weekday().num_days_from_monday() as usize];
                (format!("{}月{}日 {}", self.month, day, weekday), DayPanelState::Selected(self.selected_emails()))
            }
            None => ("选择日期".to_string(), DayPanelState::NoSelection),
        };

        let mut panel = v_flex()
            .flex_1()
            .size_full()
            .p_4()
            .gap_2()
            .id("calendar-day-scroll")
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .pb_1()
                    .child(title),
            );

        match state {
            DayPanelState::NoSelection => {
                panel = panel.child(self.empty_panel(
                    IconName::Calendar,
                    "点击左侧日期查看当天邮件",
                    &theme,
                ));
            }
            DayPanelState::Selected(ref list) if list.is_empty() => {
                panel = panel.child(self.empty_panel(IconName::MailOpen, "当天没有邮件", &theme));
            }
            DayPanelState::Selected(ref list) => {
                for e in list {
                    let time = chrono::DateTime::from_timestamp(e.sent_at, 0)
                        .map(|dt| dt.format("%H:%M").to_string())
                        .unwrap_or_default();
                    let sender = e.sender_name.clone().unwrap_or_else(|| e.sender_email.clone());
                    let avatar_char = sender.chars().next().map(|c| c.to_string()).unwrap_or_default();
                    panel = panel.child(
                        div()
                            .id(format!("mail-card-{}", e.id))
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_3()
                            .py(px(8.))
                            .rounded(theme.radius)
                            .border_1()
                            .border_color(theme.border)
                            .hover(|s| s.border_color(theme.accent.opacity(0.5)))
                            // 发件人首字圆形头像
                            .child(
                                div()
                                    .w(px(30.))
                                    .h(px(30.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .bg(theme.accent.opacity(0.2))
                                    .text_color(theme.accent)
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(avatar_char),
                            )
                            // 主题（未读加粗+点）+ 发件人
                            .child(
                                v_flex()
                                    .flex_1()
                                    .overflow_hidden()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .line_clamp(1)
                                                    .when(!e.is_read, |d| {
                                                        d.font_weight(FontWeight::SEMIBOLD)
                                                    })
                                                    .child(e.subject.clone().unwrap_or_else(|| "（无主题）".into())),
                                            )
                                            .when(!e.is_read, |d| {
                                                d.child(
                                                    div()
                                                        .w(px(6.))
                                                        .h(px(6.))
                                                        .rounded_full()
                                                        .bg(theme.accent),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(theme.muted_foreground)
                                            .line_clamp(1)
                                            .child(sender),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(theme.muted_foreground)
                                    .w(px(40.))
                                    .child(time),
                            )
                            .when(e.is_starred, |d| {
                                d.child(
                                    gpui_kit::component::Icon::new(IconName::Star)
                                        .size_3()
                                        .text_color(gpui_kit::rgb(0xf59e0b)),
                                )
                            }),
                    );
                }
            }
        }

        h_flex()
            .size_full()
            .when(!self.loaded, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_2()
                        .left_2()
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child(t("email.loading")),
                )
            })
            .child(month_panel)
            .child(panel)
    }
}

enum DayPanelState<'a> {
    NoSelection,
    Selected(Vec<&'a EmailDto>),
}

impl CalendarView {
    fn empty_panel(&self, icon: IconName, text: &str, theme: &gpui_kit::component::Theme) -> impl IntoElement {
        let _ = self;
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                gpui_kit::component::Icon::new(icon)
                    .size_8()
                    .text_color(theme.muted_foreground.opacity(0.5)),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme.muted_foreground)
                    .child(text.to_string()),
            )
    }
}

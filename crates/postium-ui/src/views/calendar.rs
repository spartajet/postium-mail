//! 日历视图（邮件时间轴）：月历充满整个内容区，作为邮件/日程的日期画布。
//! 设计：弹性大格子（行列均分填满）+ 格内邮件条目（最多 3 条 + 溢出计数）+
//! 未读强调 + 周末淡色 + 头部（导航/今天/本月统计）+ 选中日浮层详情卡片。

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

/// 格内最多展示的邮件条目数，其余折叠为“+N”。
const CELL_MAX_ITEMS: usize = 3;
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

    fn close_day(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_day = None;
        cx.notify();
    }

    /// 点击日期：再次点击已选日期则取消选择。
    fn pick_day(&mut self, day: u32, cx: &mut Context<Self>) {
        self.selected_day = if self.selected_day == Some(day) { None } else { Some(day) };
        cx.notify();
    }

    fn email_date(&self, e: &EmailDto) -> Option<NaiveDate> {
        chrono::DateTime::from_timestamp(e.sent_at, 0).map(|dt| dt.date_naive())
    }

    /// 指定日期的邮件（时间升序）。
    fn emails_of(&self, day: u32) -> Vec<&EmailDto> {
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

    /// 本月统计（总数, 未读）。
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
                    .child(format!("{}年{}月", self.year, self.month)),
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

        // ── 星期头（周末列 accent 淡色），列宽与日期格一致（均分）──
        let mut header_row = h_flex().gap(px(3.));
        for (i, w) in WEEK_LABELS.iter().enumerate() {
            header_row = header_row.child(
                div()
                    .flex_1()
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

        // ── 日期格：整月充满内容区，行列均分 ──
        // h_full：以百分比高度充满父容器（div 父容器不传播 flex_1）
        let mut grid = v_flex().h_full().min_h_0().gap(px(3.));
        let mut week = h_flex().flex_1().min_h_0().gap(px(3.));
        let mut cell_index = 0usize;
        for day in 1..=days_in_month {
            if day == 1 {
                for _ in 0..first_weekday {
                    week = week.child(div().flex_1());
                    cell_index += 1;
                }
            }
            let day_mails = self.emails_of(day);
            let unread = day_mails.iter().filter(|e| !e.is_read).count();
            let is_today = today.year() == self.year && today.month() == self.month && today.day() == day;
            let selected = self.selected_day == Some(day);
            let is_weekend = cell_index % 7 == 0 || cell_index % 7 == 6;

            let num_color = if unread > 0 {
                theme.accent
            } else if is_weekend {
                theme.muted_foreground
            } else {
                theme.foreground
            };

            let mut cell = v_flex()
                .id(("day", day))
                .flex_1()
                .h_full()
                .min_w_0()
                .overflow_hidden()
                .p(px(5.))
                .rounded(theme.radius)
                .when(is_weekend && !selected, |d| d.bg(theme.muted.opacity(0.35)))
                .when(selected, |d| {
                    d.bg(theme.accent.opacity(0.16))
                        .border_1()
                        .border_color(theme.accent)
                })
                .when(is_today && !selected, |d| d.border_1().border_color(theme.accent.opacity(0.7)))
                .when(!selected && !is_today, |d| d.hover(|s| s.bg(theme.accent.opacity(0.10))))
                .cursor_pointer()
                .on_click({
                    let day = day;
                    cx.listener(move |view: &mut Self, _, _, cx| {
                        view.pick_day(day, cx);
                    })
                })
                // 日期行：数字 + 未读点
                .child(
                    h_flex()
                        .flex_none()
                        .items_center()
                        .gap_1()
                        .pb(px(2.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(if unread > 0 || is_today { FontWeight::BOLD } else { FontWeight::NORMAL })
                                .text_color(num_color)
                                .child(day.to_string()),
                        )
                        .when(unread > 0, |d| {
                            d.child(
                                div()
                                    .w(px(5.))
                                    .h(px(5.))
                                    .rounded_full()
                                    .bg(theme.accent),
                            )
                        }),
                );

            // 格内邮件条目（最多 CELL_MAX_ITEMS 条 + 溢出计数），为将来日程条目留位
            for e in day_mails.iter().take(CELL_MAX_ITEMS) {
                cell = cell.child(
                    h_flex()
                        .flex_none()
                        .items_center()
                        .gap_1()
                        .mt(px(2.))
                        .px(px(4.))
                        .py(px(1.5))
                        .rounded(px(4.))
                        .bg(if e.is_read {
                            theme.muted.opacity(0.6)
                        } else {
                            theme.accent.opacity(0.20)
                        })
                        .child(
                            div()
                                .text_size(px(10.5))
                                .line_clamp(1)
                                .text_color(if e.is_read {
                                    theme.muted_foreground
                                } else {
                                    theme.foreground
                                })
                                .when(!e.is_read, |d| d.font_weight(FontWeight::SEMIBOLD))
                                .child(e.subject.clone().unwrap_or_else(|| "（无主题）".into())),
                        ),
                );
            }
            if day_mails.len() > CELL_MAX_ITEMS {
                cell = cell.child(
                    div()
                        .flex_none()
                        .mt(px(2.))
                        .px(px(4.))
                        .text_size(px(10.))
                        .text_color(theme.muted_foreground)
                        .child(format!("+{} 更多", day_mails.len() - CELL_MAX_ITEMS)),
                );
            }

            week = week.child(cell);
            cell_index += 1;
            if cell_index % 7 == 0 {
                grid = grid.child(week);
                week = h_flex().flex_1().min_h_0().gap(px(3.));
            }
        }
        // 月末补齐最后一行
        if cell_index % 7 != 0 {
            while cell_index % 7 != 0 {
                week = week.child(div().flex_1());
                cell_index += 1;
            }
            grid = grid.child(week);
        }

        // ── 选中日浮层详情卡片（右上，可关闭）──
        let detail_card = self.selected_day.map(|day| {
            let weekday_names = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
            let date = NaiveDate::from_ymd_opt(self.year, self.month, day).unwrap();
            let weekday = weekday_names[date.weekday().num_days_from_monday() as usize];
            let list = self.emails_of(day);

            let mut card = v_flex()
                .id("day-detail-card")
                .absolute()
                .top(px(8.))
                .right(px(8.))
                .w(px(360.))
                .h(px(460.))
                .p_3()
                .gap_2()
                .rounded(px(10.))
                .bg(theme.sidebar)
                .border_1()
                .border_color(theme.border)
                // 卡片体吞掉鼠标按下，阻止穿透到底下的日期格
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    h_flex()
                        .flex_none()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .flex_1()
                                .child(format!("{}月{}日 {}", self.month, day, weekday)),
                        )
                        .child(
                            Button::new("close-day")
                                .ghost()
                                .icon(IconName::X)
                                .on_click(cx.listener(Self::close_day)),
                        ),
                );

            if list.is_empty() {
                card = card.child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(
                            gpui_kit::component::Icon::new(IconName::MailOpen)
                                .size_8()
                                .text_color(theme.muted_foreground.opacity(0.5)),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme.muted_foreground)
                                .child("当天没有邮件"),
                        ),
                );
            } else {
                let mut list_el = v_flex().flex_1().min_h_0().gap_2().id("day-detail-scroll").overflow_y_scroll();
                for e in list {
                    let time = chrono::DateTime::from_timestamp(e.sent_at, 0)
                        .map(|dt| dt.format("%H:%M").to_string())
                        .unwrap_or_default();
                    let sender = e.sender_name.clone().unwrap_or_else(|| e.sender_email.clone());
                    let avatar_char = sender.chars().next().map(|c| c.to_string()).unwrap_or_default();
                    list_el = list_el.child(
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
                card = card.child(list_el);
            }
            card
        });

        // ── 布局：头部 + 充满内容区的月历网格 ──
        let grid_area = div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(grid);
        let grid_area = if let Some(card) = detail_card {
            grid_area.child(card)
        } else {
            grid_area
        };
        v_flex()
            .size_full()
            .p_4()
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
            .child(header)
            .child(header_row)
            .child(grid_area)
    }
}

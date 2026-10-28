//! 收件人 Chips 输入框（移植自 src/lib/components/email/RecipientField.svelte）：
//! 输入 → Enter 提交 → core `parse_email_addresses` 解析 → chip 合并去重；
//! chip 带 X 删除；无效地址就地提示（不阻断）。
//! 自动补全：ComposeView 订阅输入变更异步回填 `suggestions`，
//! 浮层候选行点击 / Enter 取第一项 → `set_value(email)` 后 commit（chips 化）。

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use postium_core::service::email_address::{ParsedEmailAddress, parse_email_addresses};
use postium_core::service::email_service::ContactDto;

pub struct RecipientField {
    /// 剩余未提交的输入（与 chips 同行展示）
    pub input: Entity<InputState>,
    /// 已确认的收件人 chips
    pub chips: Vec<ParsedEmailAddress>,
    /// 最近一次提交的无效项提示
    error: Option<String>,
    /// 自动补全候选（ComposeView 按 Change 事件防抖查询后回填）
    pub(crate) suggestions: Vec<ContactDto>,
    /// 候选浮层开关（候选非空时开，提交/清空输入后关）
    pub(crate) sugg_open: bool,
}

impl RecipientField {
    /// 创建；`initial` 非空则立即解析为 chips（回复/转发预填路径）。
    pub fn new(
        placeholder: &str,
        initial: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder.to_string())
                .submit_on_enter(true)
        });
        let mut this = Self {
            input,
            chips: Vec::new(),
            error: None,
            suggestions: Vec::new(),
            sugg_open: false,
        };
        if !initial.trim().is_empty() {
            this.commit_raw(initial.to_string(), window, cx);
        }
        this
    }

    /// 提交当前输入（Enter 或失焦触发）。
    pub fn commit(&mut self, window: &mut Window, cx: &mut App) {
        let raw = self.input.read(cx).value().to_string();
        self.commit_raw(raw, window, cx);
    }

    /// 回填自动补全候选（候选非空即开浮层）。
    pub(crate) fn set_suggestions(&mut self, list: Vec<ContactDto>) {
        self.suggestions = list;
        self.sugg_open = !self.suggestions.is_empty();
    }

    /// 关闭浮层并清空候选（提交 chips / 输入清空时）。
    pub(crate) fn close_suggestions(&mut self) {
        self.suggestions.clear();
        self.sugg_open = false;
    }

    fn commit_raw(&mut self, raw: String, window: &mut Window, cx: &mut App) {
        if raw.trim().is_empty() {
            return;
        }
        match parse_email_addresses(raw) {
            Ok(resp) => {
                for addr in resp.addresses {
                    if !self.chips.iter().any(|c| c.email.eq_ignore_ascii_case(&addr.email)) {
                        self.chips.push(addr);
                    }
                }
                if let Some(invalid) = resp.invalid.first() {
                    let msg = format!("{}：{}", invalid.raw, invalid.reason);
                    tracing::warn!(target: "postium::ui", "收件人地址无效: {msg}");
                    self.error = Some(msg);
                } else {
                    self.error = None;
                }
            }
            Err(e) => {
                tracing::warn!(target: "postium::ui", "收件人解析失败: {e}");
                self.error = Some(format!("{e}"));
            }
        }
        self.input.update(cx, |s, cx| s.set_value("", window, cx));
        // 输入已清空，候选浮层随之关闭（防抖中的在途查询由代数比对丢弃）
        self.close_suggestions();
    }

    /// 删除指定邮箱的 chip（大小写不敏感）。
    pub fn remove_by_email(&mut self, email: &str) {
        self.chips
            .retain(|c| !c.email.eq_ignore_ascii_case(email));
    }

    /// 完整文本（chips + 未提交输入），用于草稿保存。
    pub fn text(&self, cx: &App) -> String {
        let mut parts: Vec<String> = self.chips.iter().map(|c| c.raw.clone()).collect();
        let rest = self.input.read(cx).value().to_string();
        if !rest.trim().is_empty() {
            parts.push(rest);
        }
        parts.join(", ")
    }

    /// 地址列表（chips 的 email + 未提交输入按分隔符拆分），用于发送。
    pub fn emails(&self, cx: &App) -> Vec<String> {
        let mut list: Vec<String> = self.chips.iter().map(|c| c.email.clone()).collect();
        let rest = self.input.read(cx).value().to_string();
        for s in rest.split([',', ';', ' ']) {
            let s = s.trim();
            if !s.is_empty() && !list.iter().any(|x| x.eq_ignore_ascii_case(s)) {
                list.push(s.to_string());
            }
        }
        list
    }

    /// 单个 chip：显示文本 + X 删除按钮。
    fn chip_view(
        chip: &ParsedEmailAddress,
        key: usize,
        on_remove: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        theme: &gpui_kit::component::Theme,
    ) -> impl IntoElement {
        let label = match &chip.name {
            Some(n) => format!("{n} <{}>", chip.email),
            None => chip.email.clone(),
        };
        h_flex()
            .id(format!("chip-{key}"))
            .items_center()
            .gap_1()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(10.))
            .bg(theme.accent.opacity(0.14))
            .text_size(px(11.5))
            .child(div().text_color(theme.foreground).child(label))
            .child(
                div()
                    .id(format!("chip-x-{key}"))
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.foreground))
                    .child("✕")
                    .on_click(on_remove),
            )
    }

    /// 自动补全浮层中的单行候选：首字头像 + 名称 + 邮箱 + 星标★。
    /// `on_pick(idx)` 由调用方路由（set_value + commit，同 PressEnter 模式）。
    fn suggestion_view(
        c: &ContactDto,
        idx: usize,
        on_pick: std::rc::Rc<dyn Fn(usize, &ClickEvent, &mut Window, &mut App)>,
        theme: &gpui_kit::component::Theme,
    ) -> impl IntoElement {
        let display_name = c.name.clone().unwrap_or_else(|| c.email.clone());
        let initial = display_name
            .chars()
            .next()
            .map(|ch| ch.to_uppercase().to_string())
            .unwrap_or_default();
        h_flex()
            .id(format!("sugg-{idx}"))
            .items_center()
            .gap_2()
            .px_2()
            .py(px(5.))
            .cursor_pointer()
            .hover(|s| s.bg(theme.accent.opacity(0.1)))
            .child(
                div()
                    .w(px(22.))
                    .h(px(22.))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(theme.accent.opacity(0.15))
                    .text_color(theme.accent)
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .child(initial),
            )
            .child(
                div()
                    .flex_none()
                    .max_w(px(140.))
                    .text_size(px(12.))
                    .text_color(theme.foreground)
                    .line_clamp(1)
                    .child(display_name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .line_clamp(1)
                    .child(c.email.clone()),
            )
            .when(c.is_starred, |row| {
                row.child(
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(Hsla::from(gpui_kit::rgb(0xf59e0b)))
                        .child("★"),
                )
            })
            .on_click(move |e, window, cx| on_pick(idx, e, window, cx))
    }

    /// 整行渲染：chips + 内联输入框（对齐旧版同排布局）。
    /// `on_remove(email)` 由调用方路由到对应字段的删除；
    /// `on_pick(idx)` 路由到对应字段的候选提交（set_value + commit）。
    pub fn render(
        &self,
        on_remove: std::rc::Rc<dyn Fn(String, &ClickEvent, &mut Window, &mut App)>,
        on_pick: std::rc::Rc<dyn Fn(usize, &ClickEvent, &mut Window, &mut App)>,
        theme: &gpui_kit::component::Theme,
    ) -> impl IntoElement {
        let mut row = h_flex().flex_wrap().gap_1().items_center().w_full();
        for (i, chip) in self.chips.iter().enumerate() {
            let email = chip.email.clone();
            let on_remove = on_remove.clone();
            row = row.child(Self::chip_view(
                chip,
                i,
                move |e, w, cx| on_remove(email.clone(), e, w, cx),
                theme,
            ));
        }
        row = row.child(div().flex_1().min_w(px(120.)).child(Input::new(&self.input)));
        if let Some(err) = &self.error {
            row = row.child(
                div()
                    .max_w(px(240.))
                    .text_size(px(11.))
                    .text_color(gpui_kit::rgb(0xf43e5e))
                    .line_clamp(1)
                    .child(err.clone()),
            );
        }

        // relative 外层 + absolute 候选浮层（top_full 贴输入行下沿）
        div()
            .relative()
            .child(row)
            .when(self.sugg_open && !self.suggestions.is_empty(), |wrapper| {
                let mut list = v_flex()
                    .id("recipient-suggestions")
                    .absolute()
                    .top_full()
                    .left_0()
                    .w_full()
                    .mt_1()
                    .py_1()
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(8.))
                    .bg(theme.background)
                    // 浮层体吞掉鼠标按下，防止点击穿透到下方元素（学日历浮层）
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    });
                for (i, c) in self.suggestions.iter().enumerate() {
                    list = list.child(Self::suggestion_view(c, i, on_pick.clone(), theme));
                }
                wrapper.child(list)
            })
    }
}

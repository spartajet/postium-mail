//! 收件人 Chips 输入框（移植自 src/lib/components/email/RecipientField.svelte）：
//! 输入 → Enter 提交 → core `parse_email_addresses` 解析 → chip 合并去重；
//! chip 带 X 删除；无效地址就地提示（不阻断）。

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::h_flex;
use gpui_kit::*;
use postium_core::service::email_address::{ParsedEmailAddress, parse_email_addresses};

pub struct RecipientField {
    /// 剩余未提交的输入（与 chips 同行展示）
    pub input: Entity<InputState>,
    /// 已确认的收件人 chips
    pub chips: Vec<ParsedEmailAddress>,
    /// 最近一次提交的无效项提示
    error: Option<String>,
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
        let mut this = Self { input, chips: Vec::new(), error: None };
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

    /// 整行渲染：chips + 内联输入框（对齐旧版同排布局）。
    /// `on_remove(email)` 由调用方路由到对应字段的删除。
    pub fn render(
        &self,
        on_remove: std::rc::Rc<dyn Fn(String, &ClickEvent, &mut Window, &mut App)>,
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
        row
    }
}

//! HTML 正文文本划选状态机（线上反馈①：邮件正文要可选中）。
//!
//! 渲染层每个文本组（[`crate::views::email_html`] 的 flush_text_group 产物）
//! 在文档序上分得稳定序号；锚点/游标是 `(组序号, 字节偏移)`，跨组区间按
//! 文档序展开。组注册表每帧由渲染过程重建（同一缓存文档的遍历序稳定，
//! 序号帧间不漂移；换邮件时视图重建、选区随视图生灭）。
//!
//! 复制走 Ctrl+C 动作（email_detail 接线），鼠标抬起不自动写剪贴板。

use std::ops::Range;
use std::sync::{Arc, Mutex};

use postium_html::SafeUrl;

/// 跨帧共享的划选状态（渲染只读 + 鼠标事件读写）。
#[derive(Clone, Default)]
pub struct HtmlSelection {
    inner: Arc<Mutex<SelState>>,
}

#[derive(Default)]
struct SelState {
    /// 拖选中（按下未抬）。
    dragging: bool,
    anchor: Option<SelPos>,
    head: Option<SelPos>,
    /// 本帧文本组注册表（文档序）：组文本，划选复制与区间展开的依据。
    groups: Vec<String>,
    /// 右键命中上下文（右键菜单，P0）：Some=命中链接（携带 URL），None=纯
    /// 文本命中。右键按下时由 SelectableText 写入；左键按下清除（浏览器语
    /// 义：右键不破坏选区，左键开始新交互即失效）。菜单在 window.defer 的
    /// 下一帧构建——载体必须是本结构这类跨帧状态而非每帧重建的 RenderCtx。
    right_hit: Option<SafeUrl>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct SelPos {
    group: usize,
    offset: usize,
}

impl HtmlSelection {
    /// 每帧渲染开始：清空组注册表（渲染过程按序重填）。
    pub fn begin_frame(&self) {
        self.lock().groups.clear();
    }

    /// 注册一个文本组，返回组序号（文档序稳定）。
    pub fn register_group(&self, text: &str) -> usize {
        let mut state = self.lock();
        state.groups.push(text.to_string());
        state.groups.len() - 1
    }

    /// 是否有实际选区（锚点 ≠ 游标）。
    pub fn is_selecting(&self) -> bool {
        let state = self.lock();
        state.anchor.is_some_and(|a| state.head != Some(a))
    }

    /// 拖选中（移动事件只在该态更新游标）。
    pub fn is_dragging(&self) -> bool {
        self.lock().dragging
    }

    /// 按下：设锚点并进入拖选态。
    pub fn on_down(&self, group: usize, offset: usize) {
        let mut state = self.lock();
        let pos = SelPos { group, offset };
        state.dragging = true;
        state.anchor = Some(pos);
        state.head = Some(pos);
        state.right_hit = None;
    }

    /// 右键按下：记录命中上下文（Some=链接，None=纯文本）。不清选区、不进
    /// 拖选态——右键菜单（P0）读取，左键按下时自然失效。
    pub fn on_right_down(&self, link: Option<SafeUrl>) {
        self.lock().right_hit = link;
    }

    /// 双击词选（浏览器语义，P0 续）：围绕字节偏移扩张到「词字符」连串边界。
    /// 词字符 = 字母数字（含 CJK）+ `@ . _ - + #`（邮箱地址/域名/标签整体
    /// 选中，浏览器同款）；标点/空白自成连串（双击空格选空格段）。偏移在
    /// 字符边界上（index_for_position 保证）；不进拖选态（词级拖选扩展
    /// 不在本期）。
    pub fn select_word(&self, group: usize, offset: usize) {
        let mut state = self.lock();
        let Some(text) = state.groups.get(group) else {
            return;
        };
        let (start, end) = word_bounds(text, offset);
        state.dragging = false;
        state.anchor = Some(SelPos { group, offset: start });
        state.head = Some(SelPos { group, offset: end });
    }

    /// 三击段选（浏览器语义）：整组（段落级文本块）。
    pub fn select_group(&self, group: usize) {
        let mut state = self.lock();
        let Some(text) = state.groups.get(group) else {
            return;
        };
        let len = text.len();
        state.dragging = false;
        state.anchor = Some(SelPos { group, offset: 0 });
        state.head = Some(SelPos { group, offset: len });
    }

    /// 右键菜单构建期读取：Some(&SafeUrl)=链接命中（链接菜单），None=文本
    /// 命中（文本菜单）。
    pub fn right_click_link(&self) -> Option<SafeUrl> {
        self.lock().right_hit.clone()
    }

    /// 全选（右键菜单 P0）：锚点=文档首，游标=末组末偏移。组注册表为空
    /// （无文本组）时无操作。
    pub fn select_all(&self) {
        let mut state = self.lock();
        let Some((last_ix, last_len)) = state
            .groups
            .last()
            .map(|text| (state.groups.len() - 1, text.len()))
        else {
            return;
        };
        state.dragging = false;
        state.anchor = Some(SelPos {
            group: 0,
            offset: 0,
        });
        state.head = Some(SelPos {
            group: last_ix,
            offset: last_len,
        });
    }

    /// 拖动：更新游标（仅拖选态；组内偏移钳制到文本长度）。
    pub fn on_move(&self, group: usize, offset: usize) {
        let mut state = self.lock();
        if !state.dragging {
            return;
        }
        let len = state.groups.get(group).map_or(0, |t| t.len());
        state.head = Some(SelPos {
            group,
            offset: offset.min(len),
        });
    }

    /// 抬起：结束拖选。锚点 == 游标（单击）清空选区。
    pub fn on_up(&self) {
        let mut state = self.lock();
        state.dragging = false;
        if state.anchor == state.head {
            state.anchor = None;
            state.head = None;
        }
    }

    /// 清空选区（Esc / 邮件切换语义）。
    pub fn clear(&self) {
        let mut state = self.lock();
        state.dragging = false;
        state.anchor = None;
        state.head = None;
    }

    /// 组的选中字节区间（未选中/无选区 → None；整组选中 → 0..len）。
    pub fn range_for_group(&self, group: usize) -> Option<Range<usize>> {
        let state = self.lock();
        let (start, end) = ordered(&state)?;
        let len = state.groups.get(group).map_or(0, |t| t.len());
        if group < start.group || group > end.group {
            return None;
        }
        if start.group == end.group {
            return (start.offset < end.offset).then(|| start.offset..end.offset);
        }
        let range = match (group == start.group, group == end.group) {
            (true, false) => start.offset..len,
            (false, true) => 0..end.offset,
            _ => 0..len,
        };
        (range.start < range.end).then_some(range)
    }

    /// 选区文本（跨组按文档序拼接，组间换行；无选区 → None）。
    pub fn selected_text(&self) -> Option<String> {
        let state = self.lock();
        let (start, end) = ordered(&state)?;
        let mut out = String::new();
        for group in start.group..=end.group {
            let text = state.groups.get(group)?;
            if !out.is_empty() {
                out.push('\n');
            }
            let (lo, hi) = match (group == start.group, group == end.group) {
                (true, false) => (start.offset, text.len()),
                (false, true) => (0, end.offset),
                (true, true) if start.offset < end.offset => (start.offset, end.offset),
                (true, true) => (end.offset, start.offset),
                _ => (0, text.len()),
            };
            // 字节偏移由 index_for_position 给出，必在字符边界上
            out.push_str(text.get(lo..hi)?);
        }
        (!out.is_empty()).then_some(out)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SelState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// 锚点/游标的文档序（起 → 止）。
fn ordered(state: &SelState) -> Option<(SelPos, SelPos)> {
    let (a, h) = (state.anchor?, state.head?);
    Some(if a <= h { (a, h) } else { (h, a) })
}

/// 词字符：字母数字（含 CJK）+ 邮箱/域名/标签常见连接符（浏览器双击
/// 语义——`a@b.com` 整体一词）。
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '@' | '.' | '_' | '-' | '+' | '#')
}

/// 文本中围绕 `offset` 的同词类连串字节区间（start..end，start ≤ offset ≤
/// end，均在字符边界；offset==len 时取末串）。四类：ASCII 词 / CJK 词 /
/// 空白 / 其他标点——混合文本（`前缀MenuLinkText`）双击只选 ASCII 词或
/// CJK 连串（浏览器同款）；逗号与空格不相邻合并。
fn word_bounds(text: &str, offset: usize) -> (usize, usize) {
    #[derive(PartialEq, Clone, Copy)]
    enum CharClass {
        AsciiWord,
        CjkWord,
        Space,
        Punct,
    }
    let class_of = |c: char| {
        if is_word_char(c) {
            if c.is_ascii() {
                CharClass::AsciiWord
            } else {
                CharClass::CjkWord
            }
        } else if c.is_whitespace() {
            CharClass::Space
        } else {
            CharClass::Punct
        }
    };
    let offset = offset.min(text.len());
    // offset 落在字符边界；取覆盖它的字符定词类（边界上取左侧字符，即
    // 光标在词尾时仍选中该词——浏览器同款）。
    let class_at = |byte: usize| -> Option<CharClass> {
        if byte == 0 {
            text.chars().next().map(class_of)
        } else {
            text[..byte].chars().next_back().map(class_of)
        }
    };
    let Some(class) = class_at(offset) else {
        return (0, 0);
    };
    let same_class = |c: char| class_of(c) == class;
    let mut start = offset;
    while start > 0 {
        let Some(prev) = text[..start].chars().next_back() else {
            break;
        };
        if !same_class(prev) {
            break;
        }
        start -= prev.len_utf8();
    }
    let mut end = offset;
    while end < text.len() {
        let Some(next) = text[end..].chars().next() else {
            break;
        };
        if !same_class(next) {
            break;
        }
        end += next.len_utf8();
    }
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (HtmlSelection, [usize; 3]) {
        let sel = HtmlSelection::default();
        sel.begin_frame();
        let ids = [
            sel.register_group("第一组文本"),
            sel.register_group("第二组"),
            sel.register_group("第三组尾巴"),
        ];
        (sel, ids)
    }

    #[test]
    fn single_group_partial_range_and_text() {
        let (sel, ids) = setup();
        // "第一组文本" 每汉字 3 字节：6..12 = "组文"
        sel.on_down(ids[0], 6);
        sel.on_move(ids[0], 12);
        sel.on_up();
        assert!(sel.is_selecting());
        assert_eq!(sel.range_for_group(ids[0]), Some(6..12));
        assert_eq!(sel.range_for_group(ids[1]), None);
        assert_eq!(sel.selected_text().as_deref(), Some("组文"));
    }

    #[test]
    fn cross_groups_full_middle_and_text_join() {
        let (sel, ids) = setup();
        sel.on_down(ids[0], 3);
        sel.on_move(ids[2], 9);
        sel.on_up();
        // 起组尾部 + 中组整组 + 止组头部
        assert_eq!(sel.range_for_group(ids[0]), Some(3..15));
        assert_eq!(sel.range_for_group(ids[1]), Some(0..9));
        assert_eq!(sel.range_for_group(ids[2]), Some(0..9));
        assert_eq!(
            sel.selected_text().as_deref(),
            Some("一组文本\n第二组\n第三组")
        );
    }

    #[test]
    fn backward_drag_normalizes() {
        let (sel, ids) = setup();
        // 从后往前拖：文档序归一（group0[0] → group2[12]：起组整组选中）
        sel.on_down(ids[2], 12);
        sel.on_move(ids[0], 0);
        sel.on_up();
        assert_eq!(sel.range_for_group(ids[0]), Some(0..15), "起组从 0 起 → 整组");
        assert_eq!(sel.range_for_group(ids[1]), Some(0..9));
        assert_eq!(
            sel.selected_text().as_deref(),
            Some("第一组文本\n第二组\n第三组尾")
        );
    }

    #[test]
    fn click_without_drag_clears() {
        let (sel, ids) = setup();
        sel.on_down(ids[0], 3);
        sel.on_move(ids[1], 4);
        sel.on_up();
        assert!(sel.is_selecting());
        // 再单击：按下即同点 → 抬起清空
        sel.on_down(ids[0], 3);
        sel.on_up();
        assert!(!sel.is_selecting());
        assert_eq!(sel.selected_text(), None);
    }

    #[test]
    fn move_without_down_ignored_and_offset_clamped() {
        let (sel, ids) = setup();
        sel.on_move(ids[0], 5);
        sel.on_up();
        assert!(!sel.is_dragging());
        assert_eq!(sel.selected_text(), None, "无按下时移动不应产生选区");
        // 偏移钳制到组文本长度
        sel.on_down(ids[0], 0);
        sel.on_move(ids[0], 999);
        sel.on_up();
        assert_eq!(sel.selected_text().as_deref(), Some("第一组文本"));
    }

    #[test]
    fn right_hit_lifecycle() {
        let (sel, ids) = setup();
        // 右键链接命中 → 读取可见；右键纯文本 → None；右键不破坏既有选区
        sel.on_down(ids[0], 0);
        sel.on_move(ids[1], 3);
        sel.on_up();
        assert!(sel.is_selecting());
        sel.on_right_down(Some(SafeUrl::Http("https://a.test/x".into())));
        assert_eq!(
            sel.right_click_link(),
            Some(SafeUrl::Http("https://a.test/x".into()))
        );
        assert!(sel.is_selecting(), "右键不得破坏选区");
        sel.on_right_down(None);
        assert_eq!(sel.right_click_link(), None, "纯文本右键覆盖链接命中");
        // 左键按下清除命中
        sel.on_right_down(Some(SafeUrl::Http("https://b.test".into())));
        sel.on_down(ids[0], 0);
        assert_eq!(sel.right_click_link(), None, "左键按下应清除右键命中");
    }

    #[test]
    fn select_all_spans_all_groups() {
        let (sel, _ids) = setup();
        sel.select_all();
        assert!(sel.is_selecting());
        assert_eq!(
            sel.selected_text().as_deref(),
            Some("第一组文本\n第二组\n第三组尾巴"),
            "全选应覆盖首组到末组全部文本"
        );
        // 空注册表：无操作
        let empty = HtmlSelection::default();
        empty.begin_frame();
        empty.select_all();
        assert!(!empty.is_selecting());
    }

    #[test]
    fn word_selection_ascii_cjk_email_and_punct() {
        let sel = HtmlSelection::default();
        sel.begin_frame();
        // "hello world" — 各 5 字节词
        let g0 = sel.register_group("hello world");
        sel.select_word(g0, 2);
        assert_eq!(sel.selected_text().as_deref(), Some("hello"));
        sel.select_word(g0, 7);
        assert_eq!(sel.selected_text().as_deref(), Some("world"));
        // 光标在词尾边界仍选中左侧词（浏览器语义）
        sel.select_word(g0, 5);
        assert_eq!(sel.selected_text().as_deref(), Some("hello"));

        // CJK 连串一词（无空格分隔时整体选中）
        let g1 = sel.register_group("邮件双击选择");
        sel.select_word(g1, 3);
        assert_eq!(sel.selected_text().as_deref(), Some("邮件双击选择"));

        // 邮箱地址整体一词（@ . 属词字符）
        let g2 = sel.register_group("联系 a.b-c_d+e@x.y-z.com 备案");
        let email_start = "联系 ".len();
        sel.select_word(g2, email_start + 6);
        assert_eq!(sel.selected_text().as_deref(), Some("a.b-c_d+e@x.y-z.com"));

        // 连续 CJK 一词 + 标点自成连串：光标在词与逗号之间选词（浏览器
        // 同款），打在逗号右边界才选逗号
        let g3 = sel.register_group("第一, 第二");
        sel.select_word(g3, "第".len());
        assert_eq!(sel.selected_text().as_deref(), Some("第一"));
        sel.select_word(g3, "第一,".len());
        assert_eq!(sel.selected_text().as_deref(), Some(","));

        // 越界偏移安全（钳制到文本）
        sel.select_word(g3, 999);
        assert!(sel.is_selecting());

        // 混合文本（四类模型）：双击 ASCII 段只选 ASCII 词，CJK 连串独立
        let g4 = sel.register_group("前缀MenuLinkText");
        sel.select_word(g4, "前缀".len() + 4);
        assert_eq!(sel.selected_text().as_deref(), Some("MenuLinkText"));
        sel.select_word(g4, 3);
        assert_eq!(sel.selected_text().as_deref(), Some("前缀"));
    }

    #[test]
    fn triple_click_selects_whole_group() {
        let sel = HtmlSelection::default();
        sel.begin_frame();
        let g = sel.register_group("整段文本双击不中三击全选");
        sel.select_group(g);
        assert_eq!(sel.selected_text().as_deref(), Some("整段文本双击不中三击全选"));
        // 未注册组：无操作
        sel.select_group(9);
    }
}

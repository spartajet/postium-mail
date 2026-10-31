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

/// 跨帧共享的划选状态（渲染只读 + 鼠标事件读写）。
#[derive(Clone, Default)]
pub struct HtmlSelection {
    inner: Arc<Mutex<SelState>>,
}

#[derive(Default)]
struct SelState {
    /// 拖选中（按下未抬）。
    dragging: bool,
    /// 本次按下期间游标是否变化过（点击 vs 拖选判据，review I-1）。
    moved: bool,
    anchor: Option<SelPos>,
    head: Option<SelPos>,
    /// 本帧文本组注册表（文档序）：组文本，划选复制与区间展开的依据。
    groups: Vec<String>,
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

    /// 组注册表里的组文本（a11y 值等只读消费）。
    pub fn group_text(&self, group: usize) -> Option<String> {
        self.lock().groups.get(group).cloned()
    }

    /// 拖选中（移动事件只在该态更新游标）。
    pub fn is_dragging(&self) -> bool {
        self.lock().dragging
    }

    /// 按下：设锚点并进入拖选态（重置本次按下的位移标记——review I-1：
    /// 链接抬起判定要能区分"点击"与"拖选后抬起"）。
    pub fn on_down(&self, group: usize, offset: usize) {
        let mut state = self.lock();
        let pos = SelPos { group, offset };
        state.dragging = true;
        state.moved = false;
        state.anchor = Some(pos);
        state.head = Some(pos);
    }

    /// 拖动：更新游标（仅拖选态；组内偏移钳制到文本长度）。返回游标是否
    /// 变化（review I-5：调用方据此决定是否 refresh，避免拖选期每次
    /// mousemove 全窗口重排）。
    pub fn on_move(&self, group: usize, offset: usize) -> bool {
        let mut state = self.lock();
        if !state.dragging {
            return false;
        }
        let len = state.groups.get(group).map_or(0, |t| t.len());
        let new_head = SelPos {
            group,
            offset: offset.min(len),
        };
        let changed = state.head != Some(new_head);
        if changed {
            // 位移标记粘滞：拖走又拖回锚点仍是拖选（浏览器同语义）。
            // 生命周期 = 按下重置 → 移动置位 → 下次按下重置（不在 on_up
            // 清——抬起事件按注册序跑在多个文本组上，先到的 on_up 清掉
            // 标记会让后到组的链接判定读到假"未拖动"）
            state.moved = true;
            state.head = Some(new_head);
        }
        changed
    }

    /// 本次按下期间是否发生过拖动（点击 vs 拖选判据，review I-1：
    /// 在链接文字上拖选后抬起不得触发 on_link）。
    pub fn moved_during_press(&self) -> bool {
        self.lock().moved
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

    /// review I-1：moved 标记生命周期——按下重置、移动置位（粘滞：拖回
    /// 锚点仍是拖选）、下次按下重置；供链接抬起判定区分点击与拖选。
    #[test]
    fn moved_flag_distinguishes_click_from_drag() {
        let (sel, ids) = setup();
        // 纯点击：无移动 → 未拖动
        sel.on_down(ids[0], 3);
        sel.on_up();
        assert!(!sel.moved_during_press(), "原地点击不算拖动");
        // 拖选：移动过 → 拖动（拖回锚点仍粘滞）
        sel.on_down(ids[0], 3);
        assert!(sel.on_move(ids[0], 6), "游标变化应返回 true");
        assert!(!sel.on_move(ids[0], 6), "同位重复移动返回 false（review I-5：免 refresh）");
        assert!(sel.on_move(ids[0], 3), "拖回锚点仍是变化");
        assert!(sel.moved_during_press(), "拖回锚点仍是拖选");
        sel.on_up();
        assert!(sel.moved_during_press(), "抬起不清标记（多组 up 处理序安全）");
        // 下一次按下重置
        sel.on_down(ids[1], 0);
        assert!(!sel.moved_during_press(), "新按下重置");
        // 无按下时移动无效
        sel.on_up();
        assert!(!sel.on_move(ids[1], 4), "非拖选态移动返回 false 且不改状态");
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
}

# MailTextView（邮件静态 HTML 的 TextView 增强）可行性分析

> 状态：**分析完成，未获批准不实施**（用户裁定：是否做、何时做由用户决定）。
> 证据：fnd_17（现有渲染层结构）、fnd_18（GPUI 公开 API 面）、fnd_19（四项功能 API 支撑）、fnd_20（四项功能现状盘点）。

## 一、结论（TL;DR）

**可行**，且用户点名的四项功能里有 **两项现在就有、两项与重写无关**：

| 功能 | 现状 | 依据 |
|---|---|---|
| 点击链接跳转 | ✅ 已在线 | SelectableText 命中链接区间 → `on_link` → `open::that_detached` 系统处理器（email_detail.rs:491） |
| 文本划选 | ✅ 已在线 | SelectableText + HtmlSelection 状态机（组序号+字节偏移、跨组、高亮） |
| 图片右键菜单 | ❌ 缺 | 全项目无 `on_context_menu`/`is_right_click` 消费（fnd_20） |
| 图片下载 | ❌ 缺 | 无 rfd 保存对话框消费（Cargo.toml 已有 `rfd = "0.17"` 依赖） |

**关键判断：右键菜单 + 图片下载不需要重写渲染层。** 在现有元素树（`render_loaded_image` 产出的 `img()` 元素）上挂右键事件、anchored 浮层菜单、`rfd::FileDialog` 异步保存即可。这两项可以作为独立小任务先行落地，零渲染层风险。

**MailTextView 重写的真正价值**（见 §4）：统一文档级排版（真折行、行内混排、统一滚动坐标）、每帧 O(行) 而非 O(元素树) 的 paint、统一的 hit-test 命中面（图片/链接/文本共用一套 rect）。

## 二、现有渲染链路（瓶颈所在）

`crates/postium-ui/src/views/email_html.rs`（1638 行，fnd_17）：

- 架构：`postium-html` IR（sanitize 后）→ 递归 `render_html_document` → GPUI div 树。
- **文本组**：每段 flatten 成 `FlowSeg` → `flush_text_group` → `StyledText` + 自定义 `SelectableText` Element。文本组之间不能真正行内混排（`wrap_flow` 只是 flex 近似）。
- **表格**：flex 模拟（colspan→flex_grow 权重、width→basis，basis 归零交 grow 相位；构建期网格归一化统一下发列宽）。
- **图片**：三源（DataUri 缓存解码 / Cid 附件 / Remote 放行后拉取），真图 `img()`、未加载占位框。
- **链接**：InteractiveText 模式的字节区间注册，按下/抬起同区间命中触发。
- **划选**：`HtmlSelection` 锚点 =（组序号, 字节偏移），跨组区间按组序号拼接，选中段切 run 加背景色。
- 每次重渲染 = 重建整棵元素树（GPUI 无元素级 memo，DataUri 每帧重解码已是已知 P2）。

## 三、GPUI 公开 API 面（gpui-pre 0.3.9，fnd_18/fnd_19）

全部需求 API 公开可达，**无需改 gpui 源码**：

- `window.text_system()` → `Arc<WindowTextSystem>`（window.rs:2462）
- `shape_text(text, font_size, runs, wrap_width, line_clamp)` → `SmallVec<[WrappedLine;1]>` 多行折行（text_system.rs:750）；`shape_line` 单行
- `ShapedLine`：`width()/len()/paint(origin, line_height, align, align_width, window)/paint_background/paint_with_underline_handler/split_at(byte_index)`（line.rs，decoration_runs 带 background_color 高亮）
- `ShapedLineCursor::take_until(byte_index)` —— 逐段推进画笔（行内混排的文本侧）
- `window.paint_image(bounds, image_bounds, corner_radii, Arc<RenderImage>, frame_index, grayscale)`（window.rs:5053）
- `window.paint_quad / paint_path` —— 背景/边框/表格网格线
- `ClickEvent::is_right_click()/is_secondary()`（触屏长按归一）；`anchored().position(p)` + `deferred()` —— 浮层右键菜单（gpui 无现成 ContextMenu 组件，需自组，工作量在菜单 UX 不在管线）
- `ClipboardItem` 公开可构造 + `cx.write_to_clipboard`（app.rs:1750）
- 项目内先例：`SelectableText` 自实现 Element trait（request_layout 委托 / prepaint 插 hitbox / paint 接鼠标事件）——证明四方法模式可走通

## 四、MailTextView 设计草案（批准后实施）

**形态**：单个自定义 Element，吃 `HtmlDocument` IR，一次性测量缓存，paint 自绘。

```
crates/postium-ui/src/views/mail_text_view.rs（新文件，约 900-1200 行）
  MailTextView { doc, rcx }
  impl Element for MailTextView
    request_layout: 遍历 IR → LayoutRow 列表（shape_text 折行 + 图片占位 rect + 表格网格）
    prepaint: 缓存 shaped lines、hit rect 登记（链接区间/图片 rect/选区组）
    paint:    逐行 ShapedLine::paint + paint_image + paint_quad(背景/边框/表格线/选区高亮)
```

- **文本**：块内 runs 合并成单串 → `shape_text(wrap_width)` 一次折行；链接区间映射到 shaped line 字节区间；选中区间切 `DecorationRun` 背景色（与现 `apply_selection_background` 同语义）。
- **图片**：行内图以 U+FFFC 占位符进折行（宽度取图片显示宽），paint 时按占位符 x 偏移画 `paint_image`；块级图独立行。占位符宽度不可控的兜底：退化为块级（与现状一致）——这是 v1 唯一行为差（unk_5887957b 跟踪）。
- **表格**：构建期网格归一化已在 builder 完成（列宽已统一下发），MailTextView 直接按 width/colspan 算列 x 坐标，paint_quad 画竖线，单元格内递归 LayoutRow。
- **链接/划选/右键**：prepaint 登记 hit rect（链接=shaped 行内区间矩形，图片=图片矩形）；paint 阶段 `window.on_mouse_event` 消费：左键单击→链接（现语义），拖动→划选（字节偏移由 hit rect 反查），**右键→按命中对象弹出 anchored 菜单（链接：打开/复制链接；图片：另存为/复制图片；文本：复制/全选）**。
- **保存管线**（图片下载）：`rfd::FileDialog`（同步 `save_file()`，项目已有依赖）→ 后台 task 写文件（DataUri/Cid 直接写字节；Remote 用已解析 `RenderImage` 的原始帧或重新拉取 URL 字节流）。
- **引用折叠/测试探针**：保留现有 `when_probe`/`debug_selector` 键语义（`html-txt-*`/`html-img`/`html-hr` 等，e2e 依赖）；折叠块仍走现有 div 路径（混合渲染，v1 不动）。

**分期**：
1. **P0（独立，无重写依赖，建议先做）**：图片右键菜单 + 另存为 + 链接右键（打开/复制链接）+ 划选文本右键复制——挂现有元素树，估 300-450 行 + e2e。
2. **P1 MailTextView v1**：纯文本流接管（段落/标题/列表/引用文本/链接/划选），表格图片仍走旧路径，探针语义不变，估 600-800 行。
3. **P2 v2**：行内混排（图片同线）、表格自绘、深色适配验证。

**风险**：
- 工作量集中在 paint 循环与 hit-test 反查，行高/baseline 对齐需要反复对 fixture 调（无浏览器参照，靠 e2e 快照）。
- 行内图片占位符宽度问题（unk_5887957b）可能迫使 v1 保留块级图片。
- 每帧重排：MailTextView 需自管缓存（IR 指代 + 宽度指纹→LayoutRow 缓存），否则换宽度就重排——但缓存失效逻辑是纯 Rust，可单测。

## 五、建议

1. **先批 P0**（右键菜单+下载，小、独立、用户可立即感知），完成后再评估 P1/P2。
2. P1 动手前建议先写 3-5 个 fixture 快照基线（现有路径渲染），作为替换后的回归护栏。

## 六、FAQ：现在用的是 GPUI 的 TextView 吗？（2026-11-17 复核）

**不是。** 当前邮件正文渲染 = `postium-html`（sanitize → IR）→ `email_html.rs`
递归映射 GPUI div 树（文本组 `StyledText` + 自研 `SelectableText` Element），
见 fnd_17。gpui-base 0.8 的 `TextView`（`gpui_kit::base::TextView`）在立项时
评估过并否决，本轮重读 0.8.0 源码结论不变：

- **自定义 run 无注入口**：`TextViewState` 的 TextRun 管线为 `pub(super)`
  内部实现，`TextView::html(id, html)` 只吃原始 HTML 字符串，无法投喂
  已 sanitize 的 IR（email_html.rs 模块头实现记录）。
- **绕过 sanitize 边界**：其 html 解析器（text/format/html.rs，html5ever
  RcDom）直接解析原文，`<a href>` 原样进 `LinkMark`（无 SafeUrl 白名单）、
  `<img src>` 原样进 `ImageNode`——接进来等于绕开 postium-html 的 URL
  白名单与图片放行门控，违反本分支「不动 sanitize 安全边界」约束。
- **样式面窄**：inline marks 仅 bold/italic/strike/underline/code/mark/link
  （mark 仅认 color/background-color 少数色值）；邮件常见的 font-family/
  size/color/align/table colspan 等全部丢失。
- **e2e 探针依赖自研路径**：`html-txt-*`/`html-img`/`email-html-fallback`
  debug_bounds 探针由 `email_html.rs` 的 `when_probe` 产出，TextView 路径
  无对应物，切换即 20+ 条 e2e 断言失败。

0.8 新增的两个 API 让 TextView 比立项时更可用，但仍不足以接管邮件渲染：
`image_source`（图片源覆盖，权威不再回退文档 URL——可用来做放行门控）与
`on_link_click`（链接点击回调）。缺的仍是 IR 注入与富样式 run。若将来走
TextView 路线，正确姿势是 fork/上游扩展其 run 注入，而非 `html()` 直喂
原文——即 P1 自研 MailTextView 的理由依旧成立。

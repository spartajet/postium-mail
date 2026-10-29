# GPUI 邮件 HTML 渲染组件实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 自研邮件 HTML 高保真渲染组件——`crates/postium-html`（纯 Rust 管线）+ GPUI 渲染层，替换 `email_detail.rs` 现有 `html()` 文档流降级渲染。

**Architecture:** css-inline 预处理路线（外包 CSS 级联）：剥 MSO 注释 → ammonia 宽松净化 → css-inline 内联 → html5ever 解析 → 与宽度/主题无关的中间表示 `HtmlDocument`（缓存键 = message_id）→ GPUI 映射渲染（深色反转参数化）。图片三源（CID 附件/远程默认拦截/DataUri），失败一律纯文本降级。

**Tech Stack:** ammonia、css-inline、html5ever/markup5ever_rcdom、moka（postium-core）、serde_json（快照）、GPUI/gpui-kit（渲染层）。

**Spec:** docs/superpowers/specs/2026-10-28-html-renderer-design.md（本计划从 spec 论证，执行者两份都读）

## Global Constraints

- 中间表示颜色统一 `Rgba = [u8; 4]`，不引入 GPUI 类型。
- 安全白名单（树构建期终审）：href 仅 http/https/mailto；img src 仅 http/https/cid/data:image/*；违规链接降级为纯文字、违规图片产 `ImageRef::Invalid` 占位。全部属性值已由 html5ever 实体解码后再校验。
- 远程图片默认拦截；放行状态内存级、按邮件维度、不持久化。
- 深色反转纯函数放 postium-html，主题切换时算一次随文档缓存，不每帧变换。
- 降级阈值：HTML > 1MB 或节点 > 20k → 纯文本；任何层失败 → 纯文本兜底，渲染入口不得 panic 传染。
- colspan 支持（flex 权重）、rowspan 首版降级为独立行。
- commit 均提交到当前分支，中文信息，格式 `feat(html-render): …` / `fix(html-render): …`。
- 每个 crate 任务完成标准：`cargo test -p postium-html` 全绿 + `cargo build --workspace` 0 警告。

---

### Task 1: postium-html crate 骨架 + MSO 剥离 + 宽松净化

**Files:**
- Modify: 根 `Cargo.toml`（workspace members 加 `crates/postium-html`）
- Create: `crates/postium-html/Cargo.toml`（deps：ammonia、thiserror；dev：无新）
- Create: `crates/postium-html/src/lib.rs`、`src/error.rs`、`src/types.rs`、`src/sanitize.rs`

**Interfaces:**
- Produces:
  - `pub type Rgba = [u8; 4];`
  - `pub enum HtmlError { TooLarge(usize), TooManyNodes(usize), Empty }`（thiserror 中文消息）
  - `pub struct Limits { pub max_bytes: usize, pub max_nodes: usize }`（`Default` = 1MB / 20k）
  - `pub fn strip_mso_comments(html: &str) -> String`
  - `pub fn sanitize_lenient(html: &str) -> String`（ammonia：结构白名单保留 class/id/style）
  - `types.rs` 全部中间表示类型（`HtmlDocument/Block/TableRow/TableCell/InlineRun/ImageRef/SafeUrl/BlockStyle/InlineStyle/Border/Spacing/Width/Align/ImageStyle`，按 spec §3；均 `Debug/Clone/PartialEq`，`HtmlDocument` 另 derive `Serialize`）

- [ ] **Step 1: 失败测试**（`src/sanitize.rs` 内 `#[cfg(test)]`）：

```rust
#[test] fn strips_mso_conditional_comments() {
    let html = r#"<div>A<!--[if mso]><v:rect></v:rect><![endif]-->B<![if !mso]><span>C</span><![endif]></div>"#;
    let out = strip_mso_comments(html);
    assert!(!out.contains("mso") && !out.contains("v:rect"));
    assert!(out.contains("A") && out.contains("B"));
}
#[test] fn sanitize_keeps_style_attrs_drops_scripts() {
    let html = r#"<div class="a" style="color:red" onclick="x()"><script>alert(1)</script><p data-x="1">t</p></div>"#;
    let out = sanitize_lenient(html);
    assert!(out.contains(r#"class="a""#) && out.contains("color:red"));
    assert!(!out.contains("script") && !out.contains("onclick"));
}
```

- [ ] **Step 2: 跑失败**：`cargo test -p postium-html`（期望编译错误：函数不存在）
- [ ] **Step 3: 实现**：strip 用状态机扫描 `<!--`…`-->` 区间（条件注释整体删除，含 `<![if …]>…<![endif]>` 非 comment 形式）；ammonia `Builder::default().tags(白名单).tag_attributes(保留 class/id/style).url_relative(UrlRelative::PassThrough)`。依赖版本执行时 `cargo add` 最新稳定（ammonia 4.x / thiserror 2.x，对齐 workspace 现有 thiserror 版本）
- [ ] **Step 4: 跑过** + `cargo build --workspace` 0 警告
- [ ] **Step 5: Commit** `feat(html-render): postium-html crate 骨架 + MSO 剥离 + 宽松净化`

---

### Task 2: css-inline 集成 + 管线骨架

**Files:**
- Modify: `crates/postium-html/Cargo.toml`（css-inline）
- Create: `crates/postium-html/src/pipeline.rs`；`lib.rs` 挂模块

**Interfaces:**
- Consumes: Task 1 全部
- Produces:
  - `pub fn inline_css(html: &str) -> String`（css-inline 包装；单个 `<style>` 解析失败跳过继续，不整体失败）
  - `pub fn render_pipeline(html: &str) -> Result<HtmlDocument, HtmlError>`（顺序：字节数检查 → strip_mso → sanitize_lenient → inline_css → `builder::build_document`（Task 4 前 stub 为 `Ok(HtmlDocument{blocks:vec![]})`，Task 4 接管））

- [ ] **Step 1: 失败测试**：

```rust
#[test] fn inlines_style_block() {
    let html = "<style>h1{color:#ff0000}</style><h1>标题</h1>";
    let out = inline_css(html);
    assert!(out.contains("h1") && out.contains("ff0000") && !out.contains("<style>"));
}
#[test] fn malformed_style_does_not_fail() {
    let out = inline_css("<style>h1{color:</style><p>t</p>");
    assert!(out.contains("<p>t</p>"));
}
#[test] fn oversized_input_rejected() {
    let big = format!("<div>{}</div>", "x".repeat(1024 * 1024 + 10));
    assert!(matches!(render_pipeline(&big), Err(HtmlError::TooLarge(_))));
}
```

- [ ] **Step 2: 跑失败**
- [ ] **Step 3: 实现**（css-inline `CssInline::options(inline_css_options).one_inline/embedded`——用其"内联全部 style 块"API；失败 catch 后原样返回）
- [ ] **Step 4: 跑过**
- [ ] **Step 5: Commit** `feat(html-render): css-inline 集成 + 管线骨架`

---

### Task 3: 行内样式解析映射表（style.rs）

**Files:**
- Create: `crates/postium-html/src/style.rs`

**Interfaces:**
- Consumes: `types.rs` 的 BlockStyle/InlineStyle/Rgba
- Produces:
  - `pub fn parse_color(s: &str) -> Option<Rgba>`（#rgb/#rrggbb/#rrggbbaa/rgb(a())/常见色名表 ≥ 16）
  - `pub fn parse_block_style(css_text: &str) -> BlockStyle`（含 margin/padding 简写 1-4 值展开、border-width/border-color、width、display:none → `hidden: bool` 标记）
  - `pub fn parse_inline_style(css_text: &str) -> InlineStyle`（color/font-size(pt/%→px 按基准 16px)/font-weight(bold|数值≥600)/font-style:italic/text-decoration）
  - `pub fn merge_inline(base: &InlineStyle, over: &InlineStyle) -> InlineStyle`（行内嵌套叠加，None 字段透传）

- [ ] **Step 1: 失败测试**（表驱动）：

```rust
#[test] fn colors() {
    assert_eq!(parse_color("#f00"), Some([255,0,0,255]));
    assert_eq!(parse_color("#ff000080"), Some([255,0,0,128]));
    assert_eq!(parse_color("red"), Some([255,0,0,255]));
    assert_eq!(parse_color("rgb(1,2,3)"), Some([1,2,3,255]));
}
#[test] fn margin_shorthand_expands() {
    let s = parse_block_style("margin: 10px 5px");
    assert_eq!(s.margin, [10.0, 5.0, 10.0, 5.0]);
}
#[test] fn inline_merge_overrides_partial() { /* base 红色 + over bold → 红+粗；over 无色 → 保持 base */ }
```

- [ ] **Step 2: 跑失败**；**Step 3: 实现**（style 属性按 `;` 分割 + `:` 取值，不引 cssparser——css-inline 后只剩行内样式，格式规整）；**Step 4: 跑过**；**Step 5: Commit** `feat(html-render): 行内样式解析映射表`

---

### Task 4: 树构建——块级结构（builder.rs 上半）

**Files:**
- Modify: `crates/postium-html/Cargo.toml`（html5ever、markup5ever_rcdom）
- Create: `crates/postium-html/src/builder.rs`

**Interfaces:**
- Consumes: Task 3 样式解析、`Limits`
- Produces:
  - `pub fn build_document(html: &str, limits: &Limits) -> Result<HtmlDocument, HtmlError>`（节点计数超限 → `TooManyNodes`；无有效块 → `Empty`；pipeline.rs 移除 stub 改调本函数）
  - 内部：`struct TreeBuilder`（rcdom 解析 → 递归遍历）；块级白名单表：div/center→Container、p→Paragraph、h1-h6→Heading、ul/ol+li→List、blockquote→Quote、hr→Hr、br→Br(run)、table→Table（Task 6 前按 Unknown 处理）、script/style/head 元数据跳过
  - 混合内容规则：块级子元素与裸文本混排时，连续行内内容聚合为一个 `Paragraph`；`display:none` 子树剪枝（style 属性先经 Task 3 解析）

- [ ] **Step 1: 失败测试**：

```rust
#[test] fn nested_divs_build_container_tree() { /* <div style="padding:4px"><div><p>a</p></div></div> → Container[Container[Paragraph]] */ }
#[test] fn mixed_content_aggregates_paragraph() { /* <div>t1<span>s</span><p>b</p>t2</div> → Container[Paragraph(t1+s), Paragraph(b)?? 顺序: P(t1s), P(b), P(t2)] */ }
#[test] fn lists_and_headings() { /* ul>li*2 → List{items:[P,P]}, h2 → Heading{level:2} */ }
#[test] fn display_none_pruned() { /* <div style="display:none"><p>x</p></div> → 空 */ }
#[test] fn unknown_tag_downgrades_to_container() { /* <mjml-container><p>a</p></mjml-container> → Container[P] */ }
#[test] fn too_many_nodes_rejected() { /* 合成 20k+ 节点 → Err(TooManyNodes) */ }
```

- [ ] **Step 2: 跑失败**；**Step 3: 实现**；**Step 4: 跑过**；**Step 5: Commit** `feat(html-render): 树构建块级结构（容器/段落/列表/标题/引用）`

---

### Task 5: 树构建——行内扁平化 + 链接/图片终审白名单（builder.rs 下半）

**Files:**
- Modify: `crates/postium-html/src/builder.rs`

**Interfaces:**
- Consumes: Task 3 `merge_inline`、Task 4 TreeBuilder
- Produces:
  - 行内收集：`fn collect_runs(children) -> Vec<InlineRun>`（同样式相邻 Text 合并；a→`Link{href: SafeUrl, runs}`；img→`Block::Image` 提升为块级（行内 img 也提升，简化布局））
  - `fn classify_url(raw: &str) -> Option<SafeUrl>`（http/https/mailto，大小写不敏感，前导空白 trim）
  - `fn classify_image_src(raw: &str) -> ImageRef`（http/https→Remote、`cid:`→Cid（content_id 小写化）、`data:image/*`→DataUri 解 base64、其余→`ImageRef::Invalid`）

- [ ] **Step 1: 失败测试**（安全用例必测）：

```rust
#[test] fn javascript_link_downgrades_to_text() {
    let doc = build_document(r#"<a href="javascript:alert(1)">点我</a>"#, &Limits::default()).unwrap();
    // Link 节点不存在；文字保留为纯 Text run
}
#[test] fn entity_encoded_js_scheme_rejected() {
    // html5ever 已实体解码 → 树构建收到解码后的 javascript: → 同样拒绝
    let doc = build_document(r#"<a href="javascript&#58;alert(1)">x</a>"#, &Limits::default()).unwrap();
    assert!(!json(&doc).contains("javascript"));
}
#[test] fn data_text_image_rejected_but_text_kept() { /* <img src="data:text/html,..."> → ImageRef::Invalid */ }
#[test] fn cid_and_data_image_preserved() { /* cid:image001 → Cid；data:image/png;base64,iVBOR → DataUri{bytes 解码成功} */ }
#[test] fn mailto_and_https_preserved() { /* SafeUrl::Mailto / Http */ }
```

- [ ] **Step 2: 跑失败**；**Step 3: 实现**；**Step 4: 跑过**；**Step 5: Commit** `feat(html-render): 行内扁平化 + 链接/图片资源白名单终审`

---

### Task 6: 表格构建

**Files:**
- Modify: `crates/postium-html/src/builder.rs`

**Interfaces:**
- Consumes: Task 4 TreeBuilder、Task 3 样式
- Produces: `Block::Table` 完整构建（tr→TableRow、td/th→TableCell、colspan 解析（默认 1，上限 16）、cell `width` 属性/style→`Width`、嵌套表格在 cell 内递归、rowspan 忽略（各自行内独立）、thead/tbody 透明容器）

- [ ] **Step 1: 失败测试**：

```rust
#[test] fn basic_two_row_table() { /* table>tr>td*2 → Table{rows:[Row[Cell,Cell], Row[Cell,Cell]]} */ }
#[test] fn colspan_recorded() { /* <td colspan="3"> → TableCell{colspan:3} */ }
#[test] fn nested_table_recurses_into_cell() { /* td 内 table → cell.blocks 含 Table */ }
#[test] fn rowspan_ignored_as_plain_row() { /* 带 rowspan 的表格不 panic，行结构平铺 */ }
```

- [ ] **Step 2-4: 红→实现→绿**；**Step 5: Commit** `feat(html-render): 表格构建（colspan/嵌套/宽度）`

---

### Task 7: 深色反转纯函数（adapt.rs）

**Files:**
- Create: `crates/postium-html/src/adapt.rs`

**Interfaces:**
- Consumes: `HtmlDocument`
- Produces:
  - `pub fn adapt_document(doc: &HtmlDocument, dark: bool) -> HtmlDocument`（dark=false 原样 clone）
  - `pub fn needs_dark_adapt(doc: &HtmlDocument) -> bool`（顶层容器/首个显式背景亮度 ≥ 0.6 视为浅底文档）
  - `fn remap(color: Rgba, bg: bool) -> Rgba`（bg：`L' = 1 - L` 后压到 0.10–0.25 区间保持色相；fg：亮度提升至 ≥ 0.75 保饱和度）
  - 饱和判定：HSV 的 S ≥ 0.35 且 V ∈ [0.15, 0.85] → 视为有色块不动

- [ ] **Step 1: 失败测试**（表驱动）：

```rust
#[test] fn white_bg_dark_text_adapts() { /* 白底(255)黑字(17) → 背景入 0.10-0.25 区间、文字亮度 ≥ 0.75 */ }
#[test] fn saturated_block_untouched() { /* 蓝底白字营销头图区(#1a73e8 背景) → 背景色不变 */ }
#[test] fn dark_document_untouched() { /* 深底文档 needs_dark_adapt=false → clone 原样 */ }
```

- [ ] **Step 2-4: 红→实现→绿**；**Step 5: Commit** `feat(html-render): 深色反转纯函数（亮度重映射 + 饱和色豁免）`

---

### Task 8: JSON 快照框架 + 全管线串联 + fixture 语料 + 性能冒烟

**Files:**
- Modify: `crates/postium-html/src/pipeline.rs`（render_pipeline 已含 Task 4 builder，确认串联）
- Create: `crates/postium-html/tests/snapshot.rs`、`crates/postium-html/tests/fixtures/{newsletter-table.html,outlook-export.html,business-zh.html}`、`crates/postium-html/tests/perf.rs`
- Modify: `crates/postium-html/Cargo.toml`（dev-deps：serde_json）

**Interfaces:**
- Produces: 快照测试框架（`tests/fixtures/*.html` → `assert_snapshot(name, &doc)`：serde_json 序列化与 `fixtures/snapshots/<name>.json` 逐字节比对；首次生成用 `UPDATE_SNAPSHOTS=1 cargo test` 环境开关写文件）

- [ ] **Step 1: 写 3 封 fixture**（真实结构手工构造：①两列嵌套表格 newsletter 带 `<style>` 块与 CTD 图 ②Outlook 风格 MSO 剥离后含 font 标签+行内样式 ③中文商务纯段落+引用）
- [ ] **Step 2: 失败测试**（快照文件不存在 → 测试失败并提示 UPDATE_SNAPSHOTS=1）
- [ ] **Step 3: 生成快照 + 人工审阅 JSON 结构合理性**（检查表格/链接/图片分类正确）
- [ ] **Step 4: 性能测试**：

```rust
#[test] fn pipeline_500kb_under_budget() {
    let html = 合成 500KB（约 2000 节点、50 表格、100 链接）;
    let start = std::time::Instant::now();
    render_pipeline(&html).unwrap();
    let el = start.elapsed();
    assert!(el < std::time::Duration::from_millis(1000), "debug 预算 1s，实际 {el:?}");
    // release 100ms 预算由 Task 14 在 CI/release 模式验证
}
```

- [ ] **Step 5: Commit** `feat(html-render): 全管线串联 + fixture 快照 + 性能冒烟`

---

### Task 9: GPUI 渲染层——块级与行内（email_html.rs 上半）

**Files:**
- Create: `crates/postium-ui/src/views/email_html.rs`；`views/mod.rs` 挂模块
- Modify: `crates/postium-ui/Cargo.toml`（postium-html workspace 依赖）

**Interfaces:**
- Consumes: `postium_html::{HtmlDocument, Block, InlineRun, BlockStyle, …}`
- Produces:
  - `pub fn render_html_document(doc: &HtmlDocument, dark: bool, images: ImageCtx, on_link: impl Fn(&SafeUrl) + 'static, cx: &mut App) -> AnyElement`
  - `pub struct ImageCtx { pub enabled: bool, pub resolved: Arc<Mutex<HashMap<String, Option<Arc<gpui RenderImage>>>>>, pub attachments: Vec<(String /*cid*/, Arc<…>)> }`（Task 13 扩展）
  - `fn map_block_style(style: &BlockStyle, dark: bool) -> Div`（背景/边框/间距/宽度→gpui；dark=true 时颜色经 `postium_html::adapt` 的已适配文档直接消费，此处不做二次映射）
  - 行内 run 构建：gpui-base TextRun 机制（参考 `gpui-base/src/text/text_view.rs` 现有 markdown→TextRun 用法与 `email_detail.rs` selectable 用法）；同样式段合并；`selectable(true)`

- [ ] **Step 1: 失败 e2e**（`e2e_tests.rs`）：`html_render_smoke`——构造含 Container/Paragraph/Heading/List/Quote/Hr 的文档 → `render_html_document(...)` 嵌入测试窗口视图渲染不 panic、文本内容可查询（复用现有 `add_window_view` 模式）
- [ ] **Step 2: 跑失败**；**Step 3: 实现**；**Step 4: 跑过** + workspace 0 警告；**Step 5: Commit** `feat(html-render): GPUI 块级与行内渲染`

---

### Task 10: GPUI 渲染层——表格 + 链接交互（email_html.rs 下半）

**Files:**
- Modify: `crates/postium-ui/src/views/email_html.rs`

**Interfaces:**
- Consumes: Task 9 渲染入口、`SafeUrl`
- Produces:
  - 表格：Table→`v_flex`、Row→`h_flex`、Cell→容器（colspan → `flex_grow` 权重 = colspan；cell.width→flex-basis/固定宽）
  - 链接：run 区间 InteractiveText hitbox；`on_link` 回调由调用方注入（email_detail 侧 http/mailto → `cx.open_url`）；hover 下划线 + 状态栏预览 URL（复用现有 status_bar 机制或本地 hover 行）

- [ ] **Step 1: 失败 e2e**：`html_render_table_and_link`——两列表格 + colspan 链接邮件渲染不 panic；点击链接回调收到 `SafeUrl::Http("https://…")`（测试注入捕获回调）
- [ ] **Step 2-4: 红→实现→绿**（hover/状态栏如 e2e 难断言则以实现+手测为准，测试锁定回调行为）
- [ ] **Step 5: Commit** `feat(html-render): 表格 flex 模拟 + 链接交互`

---

### Task 11: email_detail 替换 + 降级路径 + 文档缓存

**Files:**
- Modify: `crates/postium-ui/src/views/email_detail.rs`（正文分支替换 `html(body)`；新增降级提示条；文档 LRU 缓存）
- Modify: `crates/postium-ui/src/e2e_tests.rs`

**Interfaces:**
- Consumes: Task 8 `render_pipeline`、Task 9 渲染入口
- Produces:
  - `struct EmailHtmlCache`（LRU 8，键 message_id，值 `{ doc: HtmlDocument, dark_doc: OnceCell<HtmlDocument> }`；dark_doc 按需生成，spec §6/§7）
  - 正文分支：`is_html` 且 pipeline `Ok` → 渲染；`Err(TooLarge|TooManyNodes|Empty)` 或净化后空 → 纯文本降级 + 顶部提示「邮件过大/无法渲染，已简化显示」（i18n 键 `email.htmlFallback`，zh/en 两表同步加，防 key-set 守卫失败）
  - 纯文本邮件路径不动；删除 `html()` 调用

- [ ] **Step 1: 失败 e2e**：`html_email_renders_via_new_renderer`（种子 HTML 邮件正文含新渲染标记文本）+ `html_email_oversize_falls_back`（构造 >1MB 正文 → 提示条出现、不 panic）
- [ ] **Step 2-4: 红→实现→绿**；**Step 5: Commit** `feat(html-render): email_detail 接入新渲染器 + 降级与缓存`

---

### Task 12: postium-core ImageFetchService + CoreApi.fetch_image

**Files:**
- Modify: `crates/postium-core/Cargo.toml`（moka）、`src/infrastructure/mod.rs`、`src/api.rs`
- Create: `crates/postium-core/src/service/image_fetch_service.rs`

**Interfaces:**
- Produces:
  - `pub struct ImageFetchService { … }`；`pub async fn fetch(&self, url: &str) -> Result<Arc<[u8]>, MailError>`（15s 超时、响应体 10MB 上限、Content-Type 前缀 `image/` 校验、moka LRU 50MB）
  - `CoreApi::fetch_image(&self, url: String) -> Result<Arc<[u8]>, MailError>`（e2e_offline 短路返回 Err——e2e 不发网络请求）

- [ ] **Step 1: 失败 wiremock 测试**（core 已有 wiremock dev-dep）：

```rust
#[tokio::test] async fn fetches_image_within_limits() { /* 200 + image/png 1KB → Ok bytes；断言缓存第二次命中不发请求（mock expect(1)） */ }
#[tokio::test] async fn rejects_non_image_content_type() { /* Content-Type: text/html → Err */ }
#[tokio::test] async fn rejects_oversize_body() { /* 11MB → Err（Content-Length 或流式截断） */ }
```

- [ ] **Step 2-4: 红→实现→绿**；**Step 5: Commit** `feat(html-render): 图片下载服务（限制 + 缓存）`

---

### Task 13: UI 图片渲染 + 「加载图片」条 + e2e

**Files:**
- Modify: `crates/postium-ui/src/views/email_html.rs`、`email_detail.rs`
- Modify: `crates/postium-ui/src/i18n_tables.rs`（`email.loadImages` zh/en）、`src/e2e_tests.rs`

**Interfaces:**
- Consumes: Task 12 `fetch_image`、Task 9 `ImageCtx`
- Produces:
  - `ImageRef` 三源渲染：`DataUri` → 立即解码（image crate 已在依赖树）显示；`Cid` → `ImageCtx.attachments` 按 content_id 匹配（`InlineAttachmentDto` 若缺 content_id 字段，本任务在 attachment_service 补查询）；`Remote`+enabled → 后台 `spawn_tokio(fetch_image)` → 解码入 `ImageCtx.resolved` → `cx.notify` 重渲染；未放行/加载中/失败/Invalid → 固定高度占位框（宽度取 style.width）
  - 「加载图片」条：正文顶部（存在 Remote 且未放行时）：说明 + 按钮 → 翻转 `HtmlRenderState.images_enabled`（email_detail 持有，按邮件维度重置）
  - e2e_offline 下 `fetch_image` Err → 占位框保留（无网络请求路径断言）

- [ ] **Step 1: 失败 e2e**：`html_images_blocked_until_allowed`——含远程图邮件打开后占位框出现、无「已加载」图像、点击「加载图片」后 `images_enabled` 翻转（e2e_offline 下保持占位）
- [ ] **Step 2-4: 红→实现→绿**；**Step 5: Commit** `feat(html-render): 图片管线 UI（拦截放行/CID/占位）`

---

### Task 14: 深色反转接线 + 语料扩充 + 性能预算 + 收尾终审

**Files:**
- Modify: `crates/postium-ui/src/views/email_html.rs`（dark 参数消费）、`email_detail.rs`（主题接入 + dark_doc 缓存）
- Modify: `crates/postium-html/tests/fixtures/`（扩至 ≥ 10 封）、`tests/perf.rs`
- Modify: `.superpowers/sdd/`（新建 html-render 工作区账本，沿用 SDD 流程）

**Interfaces:**
- Consumes: Task 7 `adapt_document`/`needs_dark_adapt`、Task 11 `EmailHtmlCache`
- Produces: 完整交付

- [ ] **Step 1: 接线**：渲染入口按当前主题取 `dark_doc`（`needs_dark_adapt && dark` 时用副本），主题切换零重解析（缓存命中）
- [ ] **Step 2: fixture 扩至 ≥10 封**（补：双层嵌套表格营销邮件、深色底原生邮件——断言 `needs_dark_adapt=false`、全图 newsletter、含 cid 内嵌图、非白名单资源攻击样本、超长中文纯文本 HTML）；快照更新并人工审阅
- [ ] **Step 3: release 模式性能验证**：`cargo test -p postium-html --release perf`（500KB < 100ms 断言在 release cfg 下启用）
- [ ] **Step 4: 全量回归**：`cargo test --workspace` + `cargo build --workspace` 0 警告 + 推送 CI 双绿
- [ ] **Step 5: Commit** `feat(html-render): 深色反转接线 + 语料扩充 + 性能预算达成`

---

## Self-Review 结论

- **Spec 覆盖**：§1 目标（T1-T14 全链）、§2 架构（T1/T2/T12 crate 划分）、§3 中间表示（T1 类型 + T4/T5/T6 构建）、§4 净化四步（T1 剥离/净化、T2 内联、T5 终审白名单）、§5 渲染层（T9/T10/T13，链接/表格/图片逐条对应）、§6 深色（T7 纯函数 + T14 接线与缓存副本）、§7 缓存与替换（T11）、§8 降级（T2 超限 + T11 降级 + T12 图片失败占位）、§9 测试（各任务内嵌 + T8 快照 + T14 语料/预算）、§10 依赖（T1/T2/T4/T12）、§11 验收（T8 快照 + T5 安全 + T13 拦截断言 + T14 回归）、§12 里程碑 M-A=T1-8 / M-B=T9-11 / M-C=T12-13 / M-D=T14。无缺口。
- **占位符**：无 TBD；依赖版本按「cargo add 最新稳定 + 对齐 workspace」明确化（ammonia/css-inline/html5ever 主版本行内给出行）。
- **类型一致性**：`render_pipeline`（T2 定义、T4 接管 stub、T8 消费）单一签名；`build_document(html, limits)`（T4 定义、T5/T6 扩展、T2 经 pipeline 调用）一致；`ImageRef::Invalid`（T5 定义、T13 消费为占位）一致；`SafeUrl`（T1 类型、T5 分类、T10 回调）一致。

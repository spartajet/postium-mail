# GPUI 邮件 HTML 渲染组件设计（postium-html）

日期：2026-10-28
状态：已确认（brainstorming 产出，方案 A）
关联：`crates/postium-ui/src/views/email_detail.rs` 现用 `html()` 文档流降级渲染器，仅认行内 style，不支持 `<style>` 块/选择器/级联，邮件表格布局保真度低。

## 1. 目标与非目标

### 目标
1. 邮件正文 HTML 高保真渲染：真实邮件的表格布局、`<style>` 块、行内样式正确呈现（目标覆盖 90% 真实邮件）。
2. 安全：对抗性输入零执行——无 JS、链接协议白名单、资源协议白名单、双层净化。
3. 隐私：远程图片（含跟踪像素）默认拦截，用户手动放行；CID 内嵌图始终显示。
4. 深色模式智能反转：白底黑字邮件自动适配深色主题，饱和色块与图片不动。
5. 性能：每邮件解析/净化/内联只做一次（按 message_id 缓存）；500KB HTML 全管线 < 100ms（release）。
6. 整棵渲染 + 编译缓存，不做虚拟化。

### 非目标
- W3C 兼容渲染器；float / absolute / media queries / flex / grid 等高级 CSS。
- 自建 CSS 级联引擎（由 css-inline 外包；后续若实测瓶颈可在管线入口替换，架构留缝）。
- webview / 嵌入式浏览器（已明确否决）。
- 超长邮件虚拟化（首版不做，超大降级纯文本兜底）。

## 2. 工程形态与总体架构

- **`crates/postium-html`**（新建，纯 Rust，不依赖 GPUI）：净化、预处理、解析、中间表示构建、颜色适配纯函数。可独立测试，未来日历描述/预览摘要可复用。
- **`postium-ui/src/views/email_html.rs`**（新文件）：中间表示 → GPUI 元素树渲染、图片加载状态、链接交互、「加载图片」按钮条。
- **`postium-core` 图片下载服务**：`ImageFetchService`（reqwest 下载 + 尺寸/超时限制 + moka 内存缓存），经 CoreApi 暴露——网络与缓存归 core，UI 不直连网络。

### 管线（方案 A：css-inline 预处理）

```
原始 HTML
 → ① 字符串层剥 MSO 条件注释
 → ② 宽松净化（ammonia：结构白名单，保留 class/id/style 供选择器匹配）
 → ③ css-inline：<style> 块按 W3C 级联计算后内联进 style 属性
 → ④ html5ever 解析 + 树遍历构建中间表示（属性级终审白名单）
 → HtmlDocument（中间表示，与宽度/主题无关）
 → ⑤ GPUI 映射渲染（每帧，便宜；深色反转参数化）
```

## 3. 中间表示

```rust
pub struct HtmlDocument { pub blocks: Vec<Block> }

pub enum Block {
    Container { style: BlockStyle, children: Vec<Block> },      // div/center 等
    Paragraph { style: BlockStyle, runs: Vec<InlineRun>, align: Align },
    Heading { level: u8, runs: Vec<InlineRun> },
    List { ordered: bool, items: Vec<Vec<Block>> },
    Table { style: BlockStyle, rows: Vec<TableRow> },
    Quote { children: Vec<Block> },
    Image { src: ImageRef, alt: String, style: ImageStyle },
    Hr,
}

pub struct TableRow { pub cells: Vec<TableCell> }
pub struct TableCell { pub blocks: Vec<Block>, pub colspan: u8, pub width: Option<Width>, pub style: BlockStyle }

pub enum InlineRun {
    Text { text: String, style: InlineStyle },
    Link { href: SafeUrl, runs: Vec<InlineRun> },
    Image { src: ImageRef, alt: String, style: ImageStyle },
    Br,
}

pub enum ImageRef {
    Remote { url: String },      // http/https，UI 决定是否加载
    Cid { content_id: String },  // 内嵌附件，始终显示
    DataUri { mime: String, bytes: Arc<[u8]> }, // 仅 image/*
}

pub enum SafeUrl { Http(String), Mailto(String) }   // 树构建期校验，其余协议直接丢弃该链接（保留文字）
```

- 颜色统一存 RGBA 元组（`rgba8`），与 GPUI 类型解耦。
- 行内嵌套（span>a>b）扁平化为 run 序列，样式叠加；同样式相邻 run 合并。
- `display:none` 子树剪枝；未知标签一律当 `Container` 处理、内容保留（宽容降级）。
- 未知行内元素当 `Text` 处理。

### 样式子集映射表（行内 style → 中间表示）

| CSS 属性 | 支持 |
|---|---|
| color / background-color | ✓（含常见颜色名 + #rgb/#rrggbb/#rrggbbaa） |
| font-size（px/%/pt）、font-weight（bold/数值）、font-style（italic）、text-decoration（underline/line-through） | ✓ |
| text-align、margin*、padding*（px/%） | ✓（auto 视为居中） |
| border / border-*（宽度 + 颜色，样式仅 solid 系） | ✓ 简化 |
| width / height（px/%） | ✓ |
| display:none | 剪枝 |
| font-family | 忽略（用主题字体，本地缺字率高） |
| position / float / flex / grid 等 | 忽略（按普通块/行内处理） |

## 4. 净化与预处理细节

1. **MSO 条件注释**：`<!--[if …]>…<![endif]-->` 与 `<![if …]>…<![endif]>` 整段字符串级删除（内部为 Outlook VML，非标准 HTML）。
2. **宽松净化（ammonia，css-inline 之前）**：标签白名单 = 邮件常见结构集（div/p/span/table/tbody/thead/tr/td/th/h1-6/ul/ol/li/a/img/br/hr/strong/em/b/i/u/font/center/blockquote/pre/sup/sub/small），保留 `class/id/style`（css-inline 选择器匹配需要），删除 script/iframe/form/object/embed/link/meta/注释事件属性。
3. **css-inline**：解析 `<style>` 块按 specificity + 源顺序级联，计算值写入每元素 `style` 属性（W3C 语义由 css-inline 保证；失败时跳过该 `<style>` 块继续，不整体失败）。
4. **终审白名单（树构建期，安全关键）**：
   - `SafeUrl`：href 仅 http/https/mailto；`javascript:`/`data:`/其他协议 → 降级为纯文字。
   - `ImageRef`：src 仅 http/https/cid/data:image/*；`data:text` 等一律拒绝 → 占位。
   - 全部属性值经 HTML 实体解码后再校验（防 `java&#115;cript:` 变体）。

## 5. GPUI 渲染层（email_html.rs）

- 块级 → `div()`（flex column）+ BlockStyle 映射（背景/边框/间距/宽度）。
- 行内 → 富文本 run 序列（复用 gpui-base 的 TextRun 基元构建，不使用其高层 html 路径）；同样式段合并后交给文本元素，`selectable(true)`。
- **链接**：run 区间注册 `InteractiveText` hitbox；Http → 系统浏览器打开；Mailto → 系统处理；hover 显示下划线 + 底部状态栏预览 URL。
- **表格**：`Table` → 嵌套 flex（table→column，row→row，cell→child 容器，嵌套表格递归）；colspan 按 flex 权重放大；rowspan 首版降级为独立行；cell `width` 转 flex-basis。
- **图片**：
  - 占位：固定高度灰框 + 图片图标（宽度尽量取 style.width），加载完成后替换，防布局跳动。
  - `Cid` → 经当前邮件详情的附件列表按 content_id 匹配（InlineAttachmentDto；若现 DTO 缺 content_id 字段，实施计划中补齐查询接口）。
  - `Remote` → images_enabled=true 时经 `CoreApi.fetch_image(url)` 下载（15s 超时、单图 10MB 上限、moka 缓存上限 50MB LRU）。
  - `DataUri` → 直接解码显示。
- **「加载图片」条**：正文顶部一行（存在 Remote 图且未放行时显示）：说明文案 + 按钮；点击翻转状态触发重渲染。放行状态按邮件维度存内存（不持久化）。

## 6. 深色模式智能反转

- 纯函数 `adapt_document(doc: &HtmlDocument, dark: bool) -> HtmlDocument`（放 postium-html，可表驱动单测）。
- 触发：dark 模式下扫描文档主背景（body/顶层容器/首个大面积背景色）亮度 ≥ 0.6。
- 动作：文字颜色亮度重映射至可读区间（暗底亮字）；背景亮度翻转（深色背景/透明背景 → 深灰体系）；**饱和色块（色度超过阈值）与图片不动**。
- 应用时机：**主题切换时计算一次并随文档缓存**（dark/light 两份按需生成），渲染映射阶段直接消费——不做每帧变换。不修改缓存的原始文档。

## 7. 缓存与替换

- **缓存键 = message_id**：中间表示与宽度无关，净化+内联+解析每邮件一次；缓存条目含原始文档 + 按需生成的深色适配副本（见 §6）。缓存存放于 email 视图层内存（LRU 8 封），不持久化。
- **替换点**：`email_detail.rs` 正文分支——纯文本邮件走现有纯文本路径；HTML 邮件改调 `render_html_document`。
- **旧路径清理**：删除对 gpui-base `html()` 的调用；gpui-base 库本身不动。

## 8. 错误处理与降级

| 情形 | 行为 |
|---|---|
| HTML > 1MB 或节点 > 20k | 纯文本降级 + 顶部提示「邮件过大，已简化显示」 |
| 净化后无有效内容 / 树构建失败 | 纯文本降级（原文本 trim） |
| css-inline 失败 | 跳过 `<style>` 继续（行内样式仍有效） |
| 图片下载失败/超时 | 占位框保留，不重试（用户可重开邮件） |
| Cid 无匹配附件 | 占位框 + 「附件缺失」 |
| 任何意外 panic 路径 | 渲染入口 catch_unwind 兜底纯文本（GPUI 渲染在帧内，禁止 panic 传染） |

## 9. 测试策略

1. **每层单测**（postium-html）：MSO 剥离、ammonia 配置、SafeUrl/ImageRef 白名单（**安全用例必测**：`javascript:` href、`java&#115;cript:` 实体变体、`data:text` 嗅探、条件注释藏毒）、样式映射表、表格构建、深色反转纯函数（表驱动）。
2. **快照测试**：中间表示 JSON 序列化快照 + 真实邮件 fixture 语料（首版手工收集 10–20 封：Outlook 导出、营销 newsletter、中文商务邮件、纯表格布局）。
3. **e2e 冒烟**（postium-ui）：HTML 邮件打开、占位框出现、「加载图片」切换、链接点击回调、降级路径触发。
4. **性能基准**：500KB HTML 全管线 < 100ms（release）；debug 放宽 10 倍记录趋势。

## 10. 依赖清单

| crate | 层 | 用途 |
|---|---|---|
| ammonia | postium-html | 宽松净化（白名单可配置） |
| css-inline | postium-html | `<style>` 级联内联（预处理核心） |
| html5ever + markup5ever_rcdom | postium-html | 容错解析（gpui-base 已用同族） |
| moka | postium-core | 图片 LRU 缓存（async 友好） |
| thiserror | postium-html | 错误类型 |

GPUI/reqwest 依赖不新增（reqwest 已在 postium-core）。

## 11. 验收标准

1. fixture 语料全部渲染并通过快照断言；三封人工判定「显著优于现有渲染器」。
2. `javascript:` 链接、非白名单资源、实体变体攻击用例全部拦截（单测覆盖）。
3. 默认状态无任何网络图片请求（e2e 断言）；放行后正常加载并缓存。
4. 深色模式下白底商务邮件可读性达标（人工验收 + 纯函数单测）。
5. `email_detail.rs` 无 `html()` 调用残留；workspace 测试全绿 0 警告。

## 12. 里程碑划分（供实施计划拆分）

- **M-A**：postium-html 全管线（①→④）+ 单测 + 快照框架（纯 Rust，可先行合入）
- **M-B**：GPUI 渲染层 + email_detail 替换 + 降级路径 + e2e 冒烟
- **M-C**：图片管线（CID 附件匹配 + ImageFetchService + 加载图片条）
- **M-D**：深色反转 + fixture 语料扩充 + 性能预算验证 + 收尾终审

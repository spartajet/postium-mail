//! 邮件详情状态：装载 → HTML 原文透传 → 渲染层三源图片管线（Task 13）。
//!
//! 图片处理已从「装载期 HTML 改写」（旧 image_injector：cid → data URI、
//! 远程图删除）迁移到渲染层三源消费：本 store 持有 [`HtmlRenderState`]
//! （放行态 + 远程拉取结果 + 内联附件解码图），正文 HTML 一律原文透传，
//! 由 `email_html::render_html_document` 按 `ImageRef` 分发。

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::service::email_service::EmailDetail;
use postium_html::{HtmlDocument, HtmlError, adapt_document, needs_dark_adapt, render_pipeline};

use crate::runtime::spawn_tokio;
use crate::ui_state;
use crate::views::email_html::{
    HtmlRenderState, collect_remote_image_urls, decode_render_image, normalize_content_id,
};

pub struct DetailStore {
    pub email_id: Option<i32>,
    pub detail: Option<EmailDetail>,
    /// 可渲染 HTML 原文（图片三源由渲染层按 ImageRef 消费，不再注入改写）
    pub html: Option<String>,
    pub loading: bool,
    /// 图片渲染状态（按邮件维度：每次装载重置；放行态的持久记忆在装载回调
    /// 按 ui-state.json 的发件人白名单恢复——线上反馈①推翻终审 M-6 的
    /// 「不持久化」裁定，Gmail「始终显示此发件人的图片」语义）。
    pub images: HtmlRenderState,
    /// 正文 HTML 文档缓存（spec §7：LRU 8，浅色/深色双通道，深色副本按需
    /// 生成）。T14 从 email_detail.rs 的进程级静态（static + Mutex）迁入：
    /// DetailStore 是 AppShell 创建一次的长寿命 Entity（app.rs 持有，跨详情
    /// 视图重建存活，T11 裁定），缓存随实体生命周期生灭，免 static 与锁。
    /// 注意：导航切换 `clear` 不清缓存——LRU 8 跨邮件翻看复用（spec §7）。
    html_cache: EmailHtmlCache,
}

#[derive(Clone, Copy)]
pub enum DetailEvent {
    Loaded,
}

impl EventEmitter<DetailEvent> for DetailStore {}

impl DetailStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            email_id: None,
            detail: None,
            html: None,
            loading: false,
            images: HtmlRenderState::default(),
            html_cache: EmailHtmlCache::default(),
        }
    }

    /// 装载一封邮件并预处理正文（远程图默认拦截，渲染层占位）。
    /// 装载详情（同 id 已装载则跳过，除非 force——服务器重载路径用）。
    pub fn load(&mut self, email_id: i32, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.load_impl(email_id, false, api, cx)
    }

    pub fn force_load(&mut self, email_id: i32, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.load_impl(email_id, true, api, cx)
    }

    /// 导航切换时清空详情（右侧不再显示旧文件夹邮件）。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.email_id = None;
        self.detail = None;
        self.html = None;
        self.loading = false;
        self.images = HtmlRenderState::default();
        cx.notify();
    }

    fn load_impl(
        &mut self,
        email_id: i32,
        force: bool,
        api: &Arc<CoreApi>,
        cx: &mut Context<Self>,
    ) {
        if self.email_id == Some(email_id) && self.detail.is_some() && !force {
            return;
        }
        self.email_id = Some(email_id);
        self.detail = None;
        self.html = None;
        self.loading = true;
        // 按邮件维度重置图片状态：放行态/已拉取图/附件解码图随旧邮件丢弃
        self.images = HtmlRenderState::default();
        cx.notify();

        let api = api.clone();
        let allow_api = api.clone();
        let rx = spawn_tokio(async move {
            let detail = api.get_email(email_id).await;
            // 内联附件字节（CID 图片数据源）：筛选+缓存落盘在 core，解码
            // 放在本任务（runtime 线程）完成，UI 线程零解码。解码失败跳过
            // （渲染层按 content_id 未匹配显示占位框）。
            let attachments = api
                .read_inline_image_bytes(email_id)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(cid, bytes)| {
                    decode_render_image(&bytes).map(|img| (normalize_content_id(&cid), img))
                })
                .collect::<Vec<_>>();
            (detail, attachments)
        });

        cx.spawn(async move |this, cx| {
            if let Ok((detail, attachments)) = rx.recv().await {
                this.update(cx, |store, cx| {
                    // 竞态防护：返回时用户已切换到其他邮件则丢弃过期结果
                    if store.email_id != Some(email_id) {
                        return;
                    }
                    match detail {
                        Ok(detail) => {
                            // 远程图片放行按发件人记忆（线上反馈①）：已放行过的
                            // 发件人自动放行并直接拉取，不再每次询问
                            let sender =
                                ui_state::normalize_sender(&detail.email.sender_email);
                            // 正文 HTML 原文透传（图片三源由渲染层按 ImageRef 消费）
                            store.html = Some(detail.body_html.clone().unwrap_or_default());
                            store.detail = Some(detail);
                            store.images.attachments = attachments;
                            if !sender.is_empty()
                                && ui_state::load().allows_remote_images_from(&sender)
                            {
                                let html = store.html.clone().unwrap_or_default();
                                // URL 清单走缓存条目（review M-5，浅色通道同键命中）
                                let urls = store.remote_image_urls(email_id, &html);
                                store.allow_remote_images(urls, &allow_api, cx);
                            }
                        }
                        Err(e) => {
                            tracing::warn!(target: "postium::ui", email_id, "加载邮件详情失败: {e}")
                        }
                    }
                    store.loading = false;
                    cx.emit(DetailEvent::Loaded);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// 放行远程图片（「加载图片」按钮，Task 13）。
    ///
    /// 翻转放行态触发重渲染；随后对 `urls`（当前文档收集的远程图 URL，
    /// 去重后）逐图后台拉取 → 解码 → 写入 `images.resolved` → notify
    /// 重渲染。resolved 键即 URL：已记录（成功/失败）的键不再重复拉取。
    /// 每个拉取结果带 email_id 竞态防护——切换邮件后的迟到结果直接丢弃。
    ///
    /// 发件人记忆（线上反馈①）：放行同时按发件人持久化（ui-state.json），
    /// 后续该发件人的邮件在装载期自动放行（见 [`DetailStore::load_impl`]）。
    pub fn allow_remote_images(
        &mut self,
        urls: Vec<String>,
        api: &Arc<CoreApi>,
        cx: &mut Context<Self>,
    ) {
        let Some(email_id) = self.email_id else {
            return;
        };
        if self.images.enabled {
            return;
        }
        self.images.enabled = true;
        if let Some(sender) = self
            .detail
            .as_ref()
            .map(|d| ui_state::normalize_sender(&d.email.sender_email))
            .filter(|s| !s.is_empty())
        {
            ui_state::remember_remote_image_sender(&sender);
        }
        cx.notify();

        for url in urls {
            // 同键去重：已解析（含失败占位）与已在途的 URL 不再重复拉取
            if self
                .images
                .resolved
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&url)
            {
                continue;
            }
            let api = api.clone();
            let rx = spawn_tokio(async move {
                // 拉取失败（含 e2e_offline 短路）→ None → resolved 记失败占位
                let bytes = api.fetch_image(url.clone()).await.ok();
                (url, bytes)
            });
            cx.spawn(async move |this, cx| {
                let Ok((url, bytes)) = rx.recv().await else {
                    return;
                };
                this.update(cx, |store, cx| {
                    // 竞态防护：已切换邮件则丢弃过期结果
                    if store.email_id != Some(email_id) {
                        return;
                    }
                    let decoded = bytes.as_deref().and_then(decode_render_image);
                    store
                        .images
                        .resolved
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(url, decoded);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// 操作后本地刷新星标态。
    pub fn apply_star(&mut self, starred: bool) {
        if let Some(d) = &mut self.detail {
            d.email.is_starred = starred;
        }
    }

    /// 取正文 HTML 文档（缓存优先，未命中解析后入缓存；深色模式取深色适配
    /// 副本）。渲染入口按当前主题传 `dark`（email_detail 接线，T14）。
    pub fn cached_doc(&mut self, email_id: i32, html: &str, dark: bool) -> Option<HtmlDocument> {
        self.html_cache.doc_for(content_key(email_id, html), html, dark)
    }

    /// 正文远程图 URL 清单（review M-5：随缓存条目一次计算，视图每帧零
    /// 重走——此前 collect_remote_image_urls 每帧全文档遍历）。管线失败
    /// （负缓存/超限）返回空清单（无 Remote 图，拦截条不显示）。
    pub fn remote_image_urls(&mut self, email_id: i32, html: &str) -> Vec<String> {
        let key = content_key(email_id, html);
        if self.html_cache.failure_of(key).is_some() {
            return Vec::new();
        }
        let doc = match self.html_cache.doc_for(key, html, false) {
            Some(doc) => doc,
            None => return Vec::new(),
        };
        let entry = match self.html_cache.map.get_mut(&key) {
            Some(CacheEntry::Doc(entry)) => entry,
            _ => unreachable!("doc_for 已保证条目为 Doc"),
        };
        entry
            .remote_urls
            .get_or_init(|| collect_remote_image_urls(&doc))
            .clone()
    }
}

// ---------------------------------------------------------------------------
// 正文 HTML 文档缓存（spec §7：LRU 8，同一邮件来回翻看零重解析；T14 自
// email_detail.rs 进程级静态迁入 DetailStore 字段，双通道 = 浅色 + 深色副本）
// ---------------------------------------------------------------------------

/// 缓存条目：浅色（原样）文档 + 深色适配文档（按需生成）。
struct CachedDoc {
    /// 浅色文档：`render_pipeline` 产出，接入时即解析。
    doc: HtmlDocument,
    /// 深色适配文档（`postium_html::adapt_document` 产出，按需一次生成）。
    /// `OnceLock` 而非 `std::cell::OnceCell`：缓存离开静态 `Mutex` 后直接
    /// 作为 Entity 字段，按 Send+Sync 口径选择（`HtmlDocument` 满足二者，
    /// `OnceLock` 随之 Sync；`get_or_init` 值语义与 OnceCell 相同）——T11
    /// 「OnceCell 口径」注记随迁移变更备案。
    dark_doc: OnceLock<HtmlDocument>,
    /// 远程图 URL 清单（review M-5：随缓存条目一次计算——此前视图每帧
    /// 全文档遍历重收集，O(doc)/帧；浅/深通道的 Remote URL 相同）。
    remote_urls: OnceLock<Vec<String>>,
}

/// 缓存条目值：管线成功产物或失败结论（终审 I-2 负缓存）。
enum CacheEntry {
    /// 管线成功：浅色 + 深色双通道文档。
    Doc(CachedDoc),
    /// 管线失败负缓存：`TooLarge`/`TooManyNodes`/`Empty` 均由 html 内容决定
    /// （同输入必同错误），失败结论入账后命中直接走调用方纯文本降级，
    /// 不再每帧全量重解析（超限邮件此前每帧跑完 html5ever parse 才报错）。
    Failed(HtmlError),
}

/// 正文 HTML 文档 LRU 缓存（容量 8，spec §7）。
///
/// 键为内容感知二元组 `(email_id, hash(html))`（评审 P2）：同 id 新正文到达
/// 后旧文档立即失效，不再以 id 命中陈旧渲染。生产同 id 换内容路径：同步
/// `UPSERT_EMAIL_SQL` 按 (account, folder, uid) 原位更新 body_html。64 位
/// 默认 hash 在 LRU 8 的空间内误撞概率可忽略。
struct EmailHtmlCache {
    /// 最近使用序（队首最旧、队尾最新，逐出从队首）。正/负缓存条目共用
    /// 同一 LRU 容量（spec §7 容量 8 的口径不因负缓存扩容）。
    order: VecDeque<CacheKey>,
    map: HashMap<CacheKey, CacheEntry>,
}

/// 缓存键：邮件 id + 正文内容 hash。
type CacheKey = (i32, u64);

/// 计算正文内容感知缓存键。
fn content_key(email_id: i32, html: &str) -> CacheKey {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    html.hash(&mut hasher);
    (email_id, hasher.finish())
}

impl Default for EmailHtmlCache {
    fn default() -> Self {
        Self {
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }
}

impl EmailHtmlCache {
    /// 缓存容量（spec §7）。
    const CAPACITY: usize = 8;

    /// 双通道取文档（T14 深色接线核心）：
    ///
    /// 1. 浅色文档：命中提升为最近使用；未命中解析后入缓存；管线失败入
    ///    负缓存（终审 I-2），返回 None 走调用方纯文本降级；
    /// 2. `dark` 且 [`needs_dark_adapt`]：深色副本 [`OnceLock::get_or_init`]
    ///    按需生成一次——主题切换零重解析（解析与适配产物都随缓存条目存活，
    ///    深底原生邮件 `needs_dark_adapt=false` 直接走浅色原文）。
    ///
    /// panic 策略：无 catch_unwind，依赖无 panic 向量构造 + GPUI 全局 hook
    /// （终审 I-1 裁定）。
    fn doc_for(&mut self, key: CacheKey, html: &str, dark: bool) -> Option<HtmlDocument> {
        self.doc_for_with(key, html, dark, render_pipeline)
    }

    /// [`Self::doc_for`] 的管线注入形态：生产走 [`render_pipeline`]，测试
    /// 注入计数器断言负缓存命中不再重跑管线（终审 I-2 专测）。
    fn doc_for_with(
        &mut self,
        key: CacheKey,
        html: &str,
        dark: bool,
        pipeline: impl Fn(&str) -> Result<HtmlDocument, HtmlError>,
    ) -> Option<HtmlDocument> {
        // 负缓存命中：失败结论内容决定性（同 html 必同错误），直接降级
        if self.failure_of(key).is_some() {
            return None;
        }
        let doc = match self.get(key) {
            Some(doc) => doc,
            None => {
                let doc = match pipeline(html) {
                    Ok(doc) => doc,
                    Err(err) => {
                        self.put_failed(key, err);
                        return None;
                    }
                };
                self.put(key, doc.clone());
                doc
            }
        };
        if !dark || !needs_dark_adapt(&doc) {
            return Some(doc);
        }
        let entry = match self.map.get_mut(&key) {
            Some(CacheEntry::Doc(entry)) => entry,
            _ => unreachable!("get/put 已保证条目为 Doc"),
        };
        Some(entry.dark_doc.get_or_init(|| adapt_document(&doc, true)).clone())
    }

    /// 负缓存查询（命中提升为最近使用）：该键已记录的管线失败结论。
    /// 错误变体随条目保留（`TooLarge`/`TooManyNodes`/`Empty` 内容决定性，
    /// 当前仅作命中判据；未来降级提示细分可直接消费）。
    fn failure_of(&mut self, key: CacheKey) -> Option<HtmlError> {
        if let Some(pos) = self.order.iter().position(|k| *k == key) {
            let k = self.order.remove(pos).expect("position 已验证存在");
            self.order.push_back(k);
        }
        match self.map.get(&key) {
            Some(CacheEntry::Failed(err)) => Some(err.clone()),
            _ => None,
        }
    }

    /// 取浅色文档（命中提升为最近使用）；未命中或负缓存条目返回 None。
    fn get(&mut self, key: CacheKey) -> Option<HtmlDocument> {
        if let Some(pos) = self.order.iter().position(|k| *k == key) {
            let k = self.order.remove(pos).expect("position 已验证存在");
            self.order.push_back(k);
        }
        match self.map.get(&key) {
            Some(CacheEntry::Doc(c)) => Some(c.doc.clone()),
            _ => None,
        }
    }

    /// 放入浅色文档（已存在则覆盖并提升）；超容量逐出最久未用。
    fn put(&mut self, key: CacheKey, doc: HtmlDocument) {
        self.insert(
            key,
            CacheEntry::Doc(CachedDoc {
                doc,
                dark_doc: OnceLock::new(),
                remote_urls: OnceLock::new(),
            }),
        );
    }

    /// 记入管线失败结论（负缓存，与正缓存共用 LRU 8 容量）。
    fn put_failed(&mut self, key: CacheKey, err: HtmlError) {
        self.insert(key, CacheEntry::Failed(err));
    }

    /// 条目写入 + LRU 容量维护（正/负缓存共用）。
    fn insert(&mut self, key: CacheKey, entry: CacheEntry) {
        if !self.order.iter().any(|k| *k == key) {
            self.order.push_back(key);
        }
        self.map.insert(key, entry);
        while self.order.len() > Self::CAPACITY {
            let evicted = self.order.pop_front().expect("长度受控非空");
            self.map.remove(&evicted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use postium_html::types::Block;

    const LIGHT_HTML: &str =
        r#"<div style="background-color:#ffffff"><p style="color:#111111">正文</p></div>"#;
    const DARK_HTML: &str =
        r#"<div style="background-color:#1e1e1e"><p style="color:#dddddd">暗色正文</p></div>"#;

    /// LRU 8 语义锁（spec §7）+ 内容感知键（评审 P2）：容量逐出最久未用、
    /// 命中提升保活、同 id 新内容 hash 不命中旧文档。
    ///（自 email_detail.rs 随缓存迁移搬入；本模块无 gpui-kit glob 导入，
    /// 裸 `#[test]` 无宏遮蔽问题。）
    #[test]
    fn cache_lru_eviction_and_content_aware_key() {
        let mut cache = EmailHtmlCache::default();
        let key = |id: i32, content: i32| (id, content as u64);
        for i in 1..=8 {
            cache.put(key(i, i), HtmlDocument::default());
        }
        // 访问 1 号（提升为最近使用），9 号入场应逐出 2 号（最久未用）
        assert!(cache.get(key(1, 1)).is_some(), "命中 1 号");
        cache.put(key(9, 9), HtmlDocument::default());
        assert!(cache.get(key(2, 2)).is_none(), "最久未用的 2 号应被逐出");
        assert!(cache.get(key(1, 1)).is_some(), "刚访问过的 1 号应保活");
        for i in 3..=9 {
            assert!(cache.get(key(i, i)).is_some(), "{i} 号应仍在缓存");
        }
        // 内容感知：同 id 换内容 hash 是不同键——旧文档不得被新正文命中
        assert!(
            cache.get(key(1, 999)).is_none(),
            "同 id 新内容不得命中旧文档"
        );
        assert!(cache.get(key(10, 10)).is_none(), "未放入的键不命中");
    }

    /// T14 深色接线核心：双通道语义——浅底文档深色通道产出适配副本（幂等、
    /// 原浅色文档不受影响）、深底原生邮件 `needs_dark_adapt=false` 深色通道
    /// 原样返回浅色文档。
    #[test]
    fn cache_dark_channel_adapts_light_and_passes_through_dark() {
        let mut cache = EmailHtmlCache::default();

        // 浅底文档：dark=false 原文、dark=true 适配副本（两者不同）
        let light_key = content_key(1, LIGHT_HTML);
        let light = cache.doc_for(light_key, LIGHT_HTML, false).expect("浅色文档");
        let dark = cache.doc_for(light_key, LIGHT_HTML, true).expect("深色副本");
        assert_ne!(light, dark, "浅底文档的深色副本应经过适配");
        // 副本幂等：第二次深色取同值（OnceLock 复用，不再重算重适配）
        assert_eq!(
            cache.doc_for(light_key, LIGHT_HTML, true).expect("再取"),
            dark
        );
        // 主题切回浅色：原浅色文档原样命中（零重解析）
        assert_eq!(
            cache.doc_for(light_key, LIGHT_HTML, false).expect("回浅"),
            light
        );

        // 深底原生邮件：深色通道返回原文（不生成副本，未做任何变换）
        let dark_key = content_key(2, DARK_HTML);
        let native = cache.doc_for(dark_key, DARK_HTML, false).expect("原文");
        assert_eq!(
            cache.doc_for(dark_key, DARK_HTML, true).expect("深色"),
            native,
            "深底文档深色通道应原样返回"
        );
    }

    /// 缓存命中零重解析：条目已存在时不走 `render_pipeline`——`""` 在管线
    /// 必 `Err`（Empty），若命中路径仍解析则 doc_for 返回 None。
    #[test]
    fn cache_hit_skips_pipeline() {
        let mut cache = EmailHtmlCache::default();
        let key = (7, 0x1234);
        let mut marker = HtmlDocument::default();
        marker.blocks.push(Block::Hr);
        cache.put(key, marker);
        let doc = cache.doc_for(key, "", false).expect("命中不应重解析");
        assert!(
            matches!(doc.blocks.as_slice(), [Block::Hr]),
            "应返回缓存条目本身，而非重新解析"
        );
    }

    /// 终审 I-2：管线失败负缓存——`""`（必 Err(Empty)）首调入账后，重取直接
    /// 降级不再重跑管线（计数器注入证明调用次数恰为 1）；负缓存条目与正缓存
    /// 共用 LRU 8，逐出后重取恢复重跑（失败结论非永久记账）。
    #[test]
    fn pipeline_failure_negative_cached_and_lru_evicted() {
        use std::cell::Cell;

        // 真实接线："" 必 Err(Empty)——首调 None + Failed 条目入账，重取仍 None
        let mut cache = EmailHtmlCache::default();
        let key = content_key(1, "");
        assert!(cache.doc_for(key, "", false).is_none());
        assert!(
            matches!(cache.map.get(&key), Some(CacheEntry::Failed(HtmlError::Empty))),
            "Empty 失败结论应入负缓存"
        );
        assert!(cache.doc_for(key, "", false).is_none());

        // 计数器注入：第二次 doc_for 不得再进管线（此前超限邮件每帧全量重解析）
        let calls = Cell::new(0u32);
        let counting = |html: &str| {
            calls.set(calls.get() + 1);
            assert!(html.contains("超限"), "注入管线只应收到测试输入");
            Err(HtmlError::TooManyNodes(1))
        };
        let key2 = content_key(2, "超限");
        assert!(cache.doc_for_with(key2, "超限", false, &counting).is_none());
        assert_eq!(calls.get(), 1, "首调应跑一次管线并记录失败");
        assert!(cache.doc_for_with(key2, "超限", false, &counting).is_none());
        assert_eq!(calls.get(), 1, "负缓存命中不得重跑管线");

        // 负缓存条目随 LRU 逐出：填满容量后再取重新跑管线（非永久记账）
        for i in 0..8 {
            cache.put(content_key(100 + i, "正文"), HtmlDocument::default());
        }
        assert!(cache.map.get(&key2).is_none(), "负缓存条目应被 LRU 逐出");
        assert!(cache.doc_for_with(key2, "超限", false, &counting).is_none());
        assert_eq!(calls.get(), 2, "逐出后重取应重新跑管线");
    }
}

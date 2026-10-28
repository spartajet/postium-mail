//! 邮件详情状态：装载 → HTML 原文透传 → 渲染层三源图片管线（Task 13）。
//!
//! 图片处理已从「装载期 HTML 改写」（旧 image_injector：cid → data URI、
//! 远程图删除）迁移到渲染层三源消费：本 store 持有 [`HtmlRenderState`]
//! （放行态 + 远程拉取结果 + 内联附件解码图），正文 HTML 一律原文透传，
//! 由 `email_html::render_html_document` 按 `ImageRef` 分发。

use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::service::email_service::EmailDetail;

use crate::runtime::spawn_tokio;
use crate::views::email_html::{HtmlRenderState, decode_render_image, normalize_content_id};

pub struct DetailStore {
    pub email_id: Option<i32>,
    pub detail: Option<EmailDetail>,
    /// 可渲染 HTML 原文（图片三源由渲染层按 ImageRef 消费，不再注入改写）
    pub html: Option<String>,
    pub loading: bool,
    /// 图片渲染状态（按邮件维度：每次装载重置——放行态不跨邮件残留，
    /// spec §4.4「放行状态按邮件维度存内存，不持久化」）。
    pub images: HtmlRenderState,
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
                    if let Ok(detail) = detail {
                        store.html = Some(detail.body_html.clone().unwrap_or_default());
                        store.detail = Some(detail);
                        store.images.attachments = attachments;
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
}

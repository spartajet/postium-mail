//! 邮件详情状态：装载 → 图片注入 → 渲染 HTML。

use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::service::email_service::EmailDetail;

use crate::image_injector::{RemoteImagePolicy, inject_images};
use crate::runtime::spawn_tokio;

pub struct DetailStore {
    pub email_id: Option<i32>,
    pub detail: Option<EmailDetail>,
    /// 注入图片后的可渲染 HTML
    pub html: Option<String>,
    pub loading: bool,
}

#[derive(Clone, Copy)]
pub enum DetailEvent {
    Loaded,
}

impl EventEmitter<DetailEvent> for DetailStore {}

impl DetailStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { email_id: None, detail: None, html: None, loading: false }
    }

    /// 装载一封邮件并预处理正文（远程图默认屏蔽）。
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
        cx.notify();
    }

    fn load_impl(&mut self, email_id: i32, force: bool, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.email_id == Some(email_id) && self.detail.is_some() && !force {
            return;
        }
        self.email_id = Some(email_id);
        self.detail = None;
        self.html = None;
        self.loading = true;
        cx.notify();

        let api = api.clone();
        let rx = spawn_tokio(async move {
            let detail = api.get_email(email_id).await;
            let inline = api.resolve_inline_attachments(email_id).await.unwrap_or_default();
            (detail, inline)
        });

        cx.spawn(async move |this, cx| {
            if let Ok((detail, inline)) = rx.recv().await {
                this.update(cx, |store, cx| {
                    // 竞态防护：返回时用户已切换到其他邮件则丢弃过期结果
                    if store.email_id != Some(email_id) {
                        return;
                    }
                    if let Ok(detail) = detail {
                        let injected = match &detail.body_html {
                            Some(html) => inject_images(html, &inline, RemoteImagePolicy::Block),
                            None => String::new(),
                        };
                        store.html = Some(injected);
                        store.detail = Some(detail);
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

    /// 操作后本地刷新星标态。
    pub fn apply_star(&mut self, starred: bool) {
        if let Some(d) = &mut self.detail {
            d.email.is_starred = starred;
        }
    }
}

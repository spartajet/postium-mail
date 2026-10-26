//! 邮件详情状态：装载 → 图片注入 → 渲染 HTML。

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, EventEmitter};
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
    pub fn load(&mut self, email_id: i32, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.email_id == Some(email_id) && self.detail.is_some() {
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

//! 同步状态（移植自 src/lib/stores/sync.svelte.ts）：事件消费 + 手动同步 + 后台调度。

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::domain::sync::{SyncEventTx, SyncProgressEvent};

use crate::runtime::spawn_tokio;

pub struct SyncStore {
    /// 各账号当前进度消息（account_id → (stage 文案, current, total)）
    pub progress: HashMap<i32, String>,
    pub syncing: bool,
}

#[derive(Clone, Copy)]
pub enum SyncEvent {
    /// 收到同步进度事件（文件夹粒度）。订阅方据此节流刷新邮件列表/徽章，
    /// 实现「边同步边更新 UI」。
    Progress,
}

impl EventEmitter<SyncEvent> for SyncStore {}

impl SyncStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { progress: HashMap::new(), syncing: false }
    }

    /// 启动事件消费循环（AppShell 装配时挂一次）。
    pub fn spawn_consumer(this: gpui_kit::WeakEntity<Self>, rx: postium_core::api::SyncEventRx, cx: &mut Context<Self>) {
        cx.spawn(async move |_, cx| {
            while let Ok(event) = rx.recv().await {
                this.update(cx, |store, cx| {
                    store.apply(event, cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub(crate) fn apply(&mut self, event: SyncProgressEvent, cx: &mut Context<Self>) {
        let p = &event.progress;
        let text = if p.total > 0 {
            format!("{}/{}", p.current, p.total)
        } else {
            p.message.clone()
        };
        let done = matches!(p.stage, postium_core::domain::sync::SyncStage::Completed);
        if done {
            self.progress.remove(&p.account_id);
        } else {
            self.progress.insert(p.account_id, text);
        }
        self.syncing = !self.progress.is_empty();
        cx.emit(SyncEvent::Progress);
    }

    /// 手动同步全部账号。
    pub fn sync_all(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        self.syncing = true;
        cx.notify();
        let api = api.clone();
        let rx = spawn_tokio(async move {
            let (tx, _rx) = smol::channel::bounded::<SyncProgressEvent>(256);
            api.sync_all_accounts(tx).await
        });
        cx.spawn(async move |this, cx| {
            // recv Err（任务失败/通道关闭）也要复位 syncing
            let result = rx.recv().await.ok();
            this.update(cx, |store, cx| {
                match result {
                    Some(Ok(_)) => {}
                    Some(Err(e)) => tracing::warn!("同步失败: {e}"),
                    None => tracing::warn!("同步任务异常终止"),
                }
                store.syncing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// 构造后台调度器的事件发送端（main.rs 接线：把 tx 传给 SyncScheduler，把 rx 交给 spawn_consumer）。
pub fn sync_event_channel() -> (SyncEventTx, postium_core::api::SyncEventRx) {
    smol::channel::bounded(256)
}

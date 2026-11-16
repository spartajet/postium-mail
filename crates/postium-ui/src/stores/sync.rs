//! 同步状态（移植自 src/lib/stores/sync.svelte.ts）：事件消费 + 手动同步 + 后台调度。

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::{Context, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::domain::sync::{SyncEventTx, SyncProgressEvent};

use crate::runtime::spawn_tokio;

pub struct SyncStore {
    /// 各账号当前进度消息（account_id → (stage 文案, current, total)）
    pub progress: HashMap<i32, String>,
    pub syncing: bool,
    /// 尚未完成首次全量同步的账号（此期间的 Completed/新增不播提示音——
    /// 首次拉历史是存量而非「新到达」；某账号收到首个非零 current 的
    /// SyncingEmails 事件即视为已完成首同步，从集合移除）。
    initial_sync_accounts: std::collections::HashSet<i32>,
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
        Self { progress: HashMap::new(), syncing: false, initial_sync_accounts: HashSet::new() }
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
            // 新邮件提示音（对照邮箱大师：收到新信响一声，独立于系统通知，MECHANICS.md §7）。
            // 首次全量拉历史（该账号本轮同步以 SyncingFolders/大批量 SyncingEmails
            // 开场）是存量而非「新到达」，不响；之后每轮增量新增>0 响一次
            // （sound 模块内置 5s 节流防多文件夹连响）。
            if p.new_emails > 0 && !self.initial_sync_accounts.remove(&p.account_id) {
                crate::sound::play_new_mail_chime();
            }
        } else {
            // 该账号本轮出现大批量同步事件（total>=50 视为存量拉取）→ 标记
            // initial，本轮 Completed 不播提示音
            if p.total >= 50 {
                self.initial_sync_accounts.insert(p.account_id);
            }
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

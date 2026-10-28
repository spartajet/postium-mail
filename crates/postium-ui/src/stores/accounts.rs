//! 账号/文件夹/标签状态（移植自 src/lib/stores/account.svelte.ts）。

use std::sync::Arc;

use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter};
use postium_core::api::CoreApi;
use postium_core::domain::sync::FolderStat;
use postium_core::service::account_service::AccountDto;
use postium_core::service::label_service::LabelDto;

use crate::runtime::spawn_tokio;

/// 一个账号的侧栏节点：账号 + 其文件夹统计 + 其标签。
#[derive(Clone)]
pub struct AccountNode {
    pub account: AccountDto,
    pub stats: Vec<FolderStat>,
    pub labels: Vec<LabelDto>,
}

pub struct AccountsStore {
    pub groups: Vec<AccountNode>,
    pub loading: bool,
    /// 在途 reload 期间又有 reload 请求：完成后补跑一次
    pending_reload: bool,
}

#[derive(Clone, Copy)]
pub enum AccountsEvent {
    Reloaded,
}

impl EventEmitter<AccountsEvent> for AccountsStore {}

impl AccountsStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { groups: Vec::new(), loading: false, pending_reload: false }
    }

    /// 异步刷新：账号列表 → 逐账号文件夹统计与标签。
    pub fn reload(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading {
            self.pending_reload = true; // 在途期间的新请求：完成后补跑
            return;
        }
        self.loading = true;
        cx.notify();

        let api = api.clone();
        let api_keep = api.clone();
        let rx = spawn_tokio(async move {
            let accounts = match api.list_accounts().await {
                Ok(accs) => accs,
                Err(e) => {
                    tracing::warn!(target: "postium::ui", "加载账号列表失败: {e}");
                    Vec::new()
                }
            };
            let mut groups = Vec::with_capacity(accounts.len());
            for account in accounts {
                let stats = api.get_folder_stats(account.id).await.unwrap_or_else(|e| {
                    tracing::warn!(target: "postium::ui", id = account.id, "加载文件夹统计失败: {e}");
                    Vec::new()
                });
                let labels = api.list_labels(account.id).await.unwrap_or_else(|e| {
                    tracing::warn!(target: "postium::ui", id = account.id, "加载标签失败: {e}");
                    Vec::new()
                });
                groups.push(AccountNode { account, stats, labels });
            }
            groups
        });

        cx.spawn(async move |this, cx| {
            let result = rx.recv().await.ok();
            this.update(cx, |store, cx| {
                if let Some(groups) = result {
                    store.groups = groups;
                }
                store.loading = false;
                cx.emit(AccountsEvent::Reloaded);
                cx.notify();
            })
            .ok();
            // 补跑挂起的 reload（若有）
            let _ = this.update(cx, |store, cx| {
                if store.pending_reload {
                    store.pending_reload = false;
                    store.reload(&api_keep, cx);
                }
            });
        })
        .detach();
    }

    /// 全部账号收件箱未读总数。
    #[allow(dead_code)] // sidebar 用免借用版本
    pub fn total_unread(&self) -> usize {
        self.groups
            .iter()
            .flat_map(|g| g.stats.iter())
            .filter(|s| s.folder.eq_ignore_ascii_case("INBOX"))
            .map(|s| s.unread)
            .sum()
    }
}

/// 在 App 上下文中创建并立即装载一次。
pub fn init_accounts_store(api: &Arc<CoreApi>, cx: &mut App) -> Entity<AccountsStore> {
    cx.new(|cx| {
        let mut store = AccountsStore::new(cx);
        store.reload(api, cx);
        store
    })
}

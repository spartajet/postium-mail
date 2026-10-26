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
}

#[derive(Clone, Copy)]
pub enum AccountsEvent {
    Reloaded,
}

impl EventEmitter<AccountsEvent> for AccountsStore {}

impl AccountsStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { groups: Vec::new(), loading: false }
    }

    /// 异步刷新：账号列表 → 逐账号文件夹统计与标签。
    pub fn reload(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();

        let api = api.clone();
        let rx = spawn_tokio(async move {
            let accounts = api.list_accounts().await.unwrap_or_default();
            let mut groups = Vec::with_capacity(accounts.len());
            for account in accounts {
                let stats = api.get_folder_stats(account.id).await.unwrap_or_default();
                let labels = api.list_labels(account.id).await.unwrap_or_default();
                groups.push(AccountNode { account, stats, labels });
            }
            groups
        });

        cx.spawn(async move |this, cx| {
            if let Ok(groups) = rx.recv().await {
                this.update(cx, |store, cx| {
                    store.groups = groups;
                    store.loading = false;
                    cx.emit(AccountsEvent::Reloaded);
                    cx.notify();
                })
                .ok();
            }
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

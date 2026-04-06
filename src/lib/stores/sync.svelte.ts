import { getContext, setContext } from 'svelte';
import { commands, events } from '$lib/bindings';
import type { SyncProgress, FolderStat } from '$lib/bindings';
import { formatError } from '$lib/utils/error.js';
import { getToastState } from '$lib/stores/toast.svelte';

class SyncState {
  syncing = $state(false);
  progress = $state<SyncProgress | null>(null);
  folderStats = $state<FolderStat[]>([]);
  error = $state<string | null>(null);

  constructor() {
    // 监听同步进度事件
    events.syncProgressEvent.listen((event) => {
      this.progress = event.payload.progress;
      if (event.payload.progress.stage === 'Completed') {
        this.syncing = false;
        try {
          const toast = getToastState();
          toast.success('同步完成');
        } catch { /* toast 可能未初始化 */ }
      } else if (event.payload.progress.stage === 'Error') {
        this.syncing = false;
        this.error = event.payload.progress.message;
        try {
          const toast = getToastState();
          toast.error('同步失败: ' + (event.payload.progress.message ?? '未知错误'));
        } catch { /* toast 可能未初始化 */ }
      }
    });
  }

  async syncAccount(accountId: number) {
    this.syncing = true;
    this.error = null;
    try {
      const result = await commands.syncAccount(accountId);
      if (result.status === 'error') {
        this.syncing = false;
        this.error = result.error.message as string;
      }
    } catch (e: unknown) {
      this.syncing = false;
      this.error = formatError(e);
    }
  }

  async loadFolderStats(accountId: number) {
    try {
      const result = await commands.getFolderStats(accountId);
      if (result.status === 'ok') {
        this.folderStats = result.data;
      }
    } catch (e: unknown) {
      console.error('Failed to load folder stats:', e);
    }
  }
}

const SYNC_KEY = Symbol('sync');

export function createSyncState() {
  const state = new SyncState();
  setContext(SYNC_KEY, state);
  return state;
}

export function getSyncState() {
  return getContext<SyncState>(SYNC_KEY);
}
